//! Package one song into a tagged Opus file: ffmpeg (WMA/WAV -> Opus transcode) -> lofty (Vorbis comments).
//!
//! The full-mix source is lossy (WMA Pro for s3p songs, MS-ADPCM WAV for the 2dx ones), neither of which has a
//! pure-Rust decoder, so a bundled `ffmpeg` transcodes it to Opus. lofty then writes the Vorbis comments and embeds the
//! per-version album jacket (fetched by `jacket.rs`) as the front cover, when one is cached for the song's version.

use crate::common::{ALBUM_ARTIST, MusicInfo, version_album_name};
use crate::unpack::AudioBlob;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result, bail};

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

// libopus target bitrate; sources are lossy (~lossy WMA / ADPCM), so 192k Opus is effectively transparent
const OPUS_BITRATE: &str = "192k";

// fail fast before a batch if ffmpeg can't be run, with a message pointing at the fix
pub fn ensure_ffmpeg() -> Result<()> {
    let path_ff = ffmpeg_path();
    Command::new(&path_ff)
        .arg("-version")
        .output()
        .with_context(|| format!("cannot run ffmpeg ({}); put ffmpeg.exe next to the program or on PATH", path_ff.display()))?;
    Ok(())
}

// transcode the audio blob to opus with ffmpeg, then attach the Vorbis tags + cover (jacket == None embeds no cover)
pub fn package(info: &MusicInfo, blob: &AudioBlob, jacket: Option<&Path>, dst_path: &Path) -> Result<()> {
    if let Some(path_parent) = dst_path.parent() {
        fs::create_dir_all(path_parent).with_context(|| format!("creating output dir {}", path_parent.display()))?;
    }

    // materialize the blob to a temp input file (ffmpeg reads files, not memory); the extension picks its demuxer
    let path_in = std::env::temp_dir().join(format!("iidxonear_in_{}.{}", info.id, blob.input_ext()));
    fs::write(&path_in, blob.bytes()).with_context(|| format!("writing temp input {}", path_in.display()))?;

    // work on a temp output in the dst folder, then atomically rename, so an interrupted run never leaves a half-written
    // `.opus` that the incremental scan would mistake for done. Temp keeps the `.opus` suffix for format detection.
    let path_tmp = dst_path.with_extension("part.opus");
    let result = transcode_opus(&path_in, &path_tmp).and_then(|()| write_tags(info, jacket, &path_tmp));

    let _ = fs::remove_file(&path_in);                                  // always clean the temp input
    if let Err(e) = result {
        let _ = fs::remove_file(&path_tmp);                             // best-effort cleanup of the partial output
        return Err(e);
    }
    fs::rename(&path_tmp, dst_path).with_context(|| format!("finalizing {}", dst_path.display()))?;
    Ok(())
}

// run ffmpeg to decode the source and encode Opus; source metadata is dropped here, lofty writes our own afterwards
fn transcode_opus(path_in: &Path, dst_path: &Path) -> Result<()> {
    let status = Command::new(ffmpeg_path())
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(path_in)
        .args(["-vn", "-map_metadata", "-1", "-c:a", "libopus", "-b:a", OPUS_BITRATE])
        .arg(dst_path)
        .status()
        .with_context(|| format!("spawning ffmpeg for {}", path_in.display()))?;
    if !status.success() {
        bail!("ffmpeg failed ({status}) converting {}", path_in.display());
    }
    Ok(())
}

// lofty: attach the Vorbis comments + optional front-cover picture to the encoded opus
fn write_tags(info: &MusicInfo, jacket: Option<&Path>, dst_path: &Path) -> Result<()> {
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_title(info.str_title.clone());                              // TITLE
    tag.set_artist(info.str_artist.clone());                            // ARTIST
    tag.set_album(version_album_name(info.version).to_string());        // ALBUM (game version name)
    insert_if_set(&mut tag, ItemKey::Genre, &info.str_genre);           // GENRE (skip when the db has none)
    tag.insert_text(ItemKey::AlbumArtist, ALBUM_ARTIST.to_string());    // ALBUMARTIST (fixed "BEMANI" for grouping)
    tag.insert_text(ItemKey::TrackNumber, (info.id % 1000 + 1).to_string());  // TRACKNUMBER (1-based within-version index)

    /*
    ****************************************************************************************************************
    内嵌封面: jacket 为该版本本地缓存的图片路径(由 jacket.rs 解析, 且调用方已确认文件存在)。MIME 由扩展名判定
    (png -> Png, 其余按 jpg)。同一版本所有歌共用该版本封面, 正好对应"按版本分专辑"。jacket==None 则不嵌(如 --skip-jacket
    或该版本无封面)。
    ****************************************************************************************************************
    */
    if let Some(path_jacket) = jacket {
        let vec_jacket = fs::read(path_jacket).with_context(|| format!("reading jacket {}", path_jacket.display()))?;
        let mime = match path_jacket.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("png") => MimeType::Png,
            _ => MimeType::Jpeg,
        };
        let picture = Picture::unchecked(vec_jacket)
            .pic_type(PictureType::CoverFront)
            .mime_type(mime)
            .build();
        tag.push_picture(picture);
    }

    tag.save_to_path(dst_path, WriteOptions::default())
        .with_context(|| format!("writing tags to {}", dst_path.display()))?;
    Ok(())
}

// insert a text field only when non-empty, to avoid writing blank Vorbis comments
fn insert_if_set(tag: &mut Tag, item_key: ItemKey, str_value: &str) {
    if !str_value.is_empty() {
        tag.insert_text(item_key, str_value.to_string());
    }
}

// locate ffmpeg: prefer one next to our own executable (release layout), then the working dir (where the user dropped
// it), else fall back to PATH. Paths must be absolute: Command on Windows searches PATH, not the cwd, for a bare name.
fn ffmpeg_path() -> PathBuf {
    if let Ok(path_exe) = std::env::current_exe() {
        if let Some(path_dir) = path_exe.parent() {
            let path_ff = path_dir.join("ffmpeg.exe");
            if path_ff.exists() {
                return path_ff;
            }
        }
    }
    if let Ok(path_cwd) = std::env::current_dir() {
        let path_ff = path_cwd.join("ffmpeg.exe");
        if path_ff.exists() {
            return path_ff;
        }
    }
    PathBuf::from("ffmpeg")
}
