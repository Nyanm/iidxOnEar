//! Plan one packaging task per output Opus.
//!
//! Id-driven: walk the valid `MusicInfo` entries (optionally filtered to selected versions), locate each song's audio
//! by its 5-digit id, and emit a `PackTask`. Audio lives either loose (`sound/<id5>/<id5>.s3p`, gen 30+) or packed
//! (`sound/<id5>.ifs`, gen < 30); omnimix-revived songs are searched under the omnimix sound dir first.

use crate::common::{AudioSource, MusicInfo, PackTask, version_folder_name};

use std::fs;
use std::path::{Path, PathBuf};

// --- incremental support: drop tasks whose output already exists --------------------------------------------------

// each task's dst_path is known up front, so an incremental run just drops the ones already on disk: a cheap stat,
// no tag reading. Called before the worker pool so the progress count reflects the real work to do.
pub fn filter_existing(vec_task: &mut Vec<PackTask>) {
    vec_task.retain(|task| !task.dst_path.exists());
}

// --- general scan: one task per song ------------------------------------------------------------------------------

// walk the valid songs (filtered by `versions` when non-empty), locate each song's audio, and plan a packaging task.
// `path_sound_omni` is the omnimix sound dir when an omnimix patch is installed, else None.
pub fn scan_songs(
    path_sound: &Path,
    path_sound_omni: Option<&Path>,
    path_out: &Path,
    vec_music: &[MusicInfo],
    versions: &[u8],
) -> Vec<PackTask> {
    let mut vec_task = Vec::new();

    for info in vec_music.iter().filter(|m| m.is_valid) {
        if !versions.is_empty() && !versions.contains(&info.version) {
            continue;                                                   // version filter (empty == convert all)
        }
        let str_id5 = format!("{:05}", info.id);

        // omni songs prefer the omnimix sound dir, normal songs prefer the base dir; the other is a fallback
        let roots: Vec<&Path> = match (info.is_omnimix, path_sound_omni) {
            (true, Some(omni)) => vec![omni, path_sound],
            (_, Some(omni)) => vec![path_sound, omni],
            (_, None) => vec![path_sound],
        };

        match locate_audio(&roots, &str_id5) {
            Some(audio_src) => vec_task.push(PackTask {
                info: info.clone(),
                audio_src,
                dst_path: build_dst(path_out, info.version, &info.str_title),
            }),
            None => eprintln!("[skip] #{} {}: audio not found ({str_id5})", info.id, info.str_title),
        }
    }

    vec_task
}

// resolve a song's audio under the given roots, trying each root in order and returning the first hit. Loose
// containers win over a packed ".ifs"; ".s3p" wins over ".2dx". None when nothing matches in any root.
fn locate_audio(roots: &[&Path], str_id5: &str) -> Option<AudioSource> {
    for root in roots {
        let path_dir = root.join(str_id5);
        let path_s3p = path_dir.join(format!("{str_id5}.s3p"));
        if path_s3p.is_file() {
            return Some(AudioSource::Loose(path_s3p));
        }
        if let Some(path_2dx) = first_main_2dx(&path_dir, str_id5) {
            return Some(AudioSource::Loose(path_2dx));
        }
        let path_ifs = root.join(format!("{str_id5}.ifs"));
        if path_ifs.is_file() {
            return Some(AudioSource::Ifs(path_ifs));
        }
    }
    None
}

// pick the main loose ".2dx" for a song: the lexicographically-first "<id5>*.2dx" in `path_dir`, excluding the
// "<id5>_pre.2dx" preview. Sorting makes a suffix-less "<id5>.2dx" win, else the lowest-suffix variant.
fn first_main_2dx(path_dir: &Path, str_id5: &str) -> Option<PathBuf> {
    let str_pre = format!("{str_id5}_pre.2dx");
    let mut vec_name: Vec<String> = fs::read_dir(path_dir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(str_id5) && n.ends_with(".2dx") && *n != str_pre)
        .collect();
    vec_name.sort();
    vec_name.into_iter().next().map(|n| path_dir.join(n))
}

// --- omnimix patch discovery --------------------------------------------------------------------------------------

// locate an "omnimix" directory within `path_root`, searching up to 2 levels deep (it sits at contents/data_mods/
// omnimix). Returns the shallowest match, or None when no patch is installed.
pub fn find_omnimix(path_root: &Path) -> Option<PathBuf> {
    find_dir(path_root, "omnimix", 2)
}

// depth-bounded search for a sub-directory with the given name (checks the current level before recursing)
fn find_dir(path_dir: &Path, str_name: &str, depth: u32) -> Option<PathBuf> {
    let mut vec_sub = Vec::new();
    for entry in fs::read_dir(path_dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(str_name) {
                return Some(path);
            }
            vec_sub.push(path);
        }
    }
    if depth == 0 {
        return None;
    }
    vec_sub.into_iter().find_map(|p| find_dir(&p, str_name, depth - 1))
}

// --- helpers ------------------------------------------------------------------------------------------------------

// destination path: <out>/<version folder>/<sanitized title>.opus
fn build_dst(path_out: &Path, version: u8, str_title: &str) -> PathBuf {
    path_out
        .join(version_folder_name(version))
        .join(format!("{}.opus", sanitize_filename(str_title)))
}

// replace characters illegal in Windows file names with '_'
fn sanitize_filename(str_title: &str) -> String {
    str_title
        .chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect()
}
