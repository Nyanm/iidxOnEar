//! Package one song into a tagged Opus file: render the full song via `iidx_on_knitting` (BMS-style keysound
//! reconstruction -> Ogg/Opus), then write the Vorbis comments and embed the per-version album jacket with lofty.
//!
//! All the heavy lifting (ifs/folder resolution, WMA/WAV keysound decode, mixing, Opus encoding) lives in
//! `iidx_on_knitting::render_song`; here we only pick a difficulty, tag the result, and finalize it atomically.

use crate::common::{ALBUM_ARTIST, MusicInfo, SongInput, version_album_name};

use std::fs;
use std::path::Path;
use anyhow::{Context, Result};

use iidx_on_knitting::{Difficulty, RenderError, convert_song, render_packed_song, render_song};
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

// any difficulty reconstructs the same song; SPN exists for nearly every song, the rest are fallbacks
const DIFFICULTY_PRIORITY: [Difficulty; 10] = [
    Difficulty::SpNormal, Difficulty::SpHyper, Difficulty::SpAnother, Difficulty::SpBeginner, Difficulty::SpLeggendaria,
    Difficulty::DpNormal, Difficulty::DpHyper, Difficulty::DpAnother, Difficulty::DpBeginner, Difficulty::DpLeggendaria,
];

// render the song to Opus via iidx_on_knitting, then attach the Vorbis tags + cover; jacket == None embeds no cover
pub fn package(info: &MusicInfo, input: &SongInput, jacket: Option<&Path>, dst_path: &Path) -> Result<()> {
    if let Some(path_parent) = dst_path.parent() {
        fs::create_dir_all(path_parent).with_context(|| format!("creating output dir {}", path_parent.display()))?;
    }

    // render to a temp Ogg/Opus next to the destination, then tag + atomically rename, so an interrupted run never
    // leaves a half-written file the incremental scan would mistake for done
    let path_tmp = dst_path.with_extension("part.ogg");
    let result = render_any_difficulty(input, &path_tmp).and_then(|()| write_tags(info, jacket, &path_tmp));
    if let Err(e) = result {
        let _ = fs::remove_file(&path_tmp);                            // best-effort cleanup of the partial output
        return Err(e);
    }
    fs::rename(&path_tmp, dst_path).with_context(|| format!("finalizing {}", dst_path.display()))?;
    Ok(())
}

fn render_any_difficulty(input: &SongInput, out_ogg: &Path) -> Result<()> {
    let mut first_err: Option<RenderError> = None;
    for difficulty in DIFFICULTY_PRIORITY {
        let result = match input {
            SongInput::Loose { audio, chart } => render_song(audio, chart, out_ogg, difficulty),
            SongInput::Packed(ifs) => render_packed_song(ifs, out_ogg, difficulty),
        };
        match result {
            Ok(()) => return Ok(()),
            // NotKeysound = the audio is pre-mixed (checked before the chart, so difficulty-independent): stop retrying
            // and return now. A loose audio file is transcoded directly; a packed .ifs has no separate file to convert.
            Err(RenderError::NotKeysound) => {
                return match input {
                    SongInput::Loose { audio, .. } => convert_song(audio, out_ogg).map_err(Into::into),
                    SongInput::Packed(_) => Err(RenderError::NotKeysound.into()),
                };
            }
            Err(e) => { first_err.get_or_insert(e); }                 // missing slot / decode failure -> try next difficulty
        }
    }
    // every difficulty failed; surface the first (SPN) error — far more useful than the last (DPL, absent for ~all songs)
    Err(first_err.expect("DIFFICULTY_PRIORITY is non-empty").into())
}

// lofty: attach the Vorbis comments + optional front-cover picture to the rendered Ogg/Opus
fn write_tags(info: &MusicInfo, jacket: Option<&Path>, dst_path: &Path) -> Result<()> {
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_title(info.str_title.clone());                              // TITLE
    tag.set_artist(info.str_artist.clone());                            // ARTIST
    tag.set_album(version_album_name(info.version).to_string());        // ALBUM (game version name)
    insert_if_set(&mut tag, ItemKey::Genre, &info.str_genre);           // GENRE (skip when the db has none)
    tag.insert_text(ItemKey::AlbumArtist, ALBUM_ARTIST.to_string());    // ALBUMARTIST (fixed "BEMANI" for grouping)
    tag.insert_text(ItemKey::TrackNumber, (info.id % 1000 + 1).to_string());  // TRACKNUMBER (1-based within-version index)

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
