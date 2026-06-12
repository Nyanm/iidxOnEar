//! Plan one packaging task per output Opus.
//!
//! Id-driven: walk the valid `MusicInfo` entries (optionally filtered to selected versions), locate each song's
//! on-disk input by its 5-digit id, and emit a `PackTask`. A song is either a loose folder `sound/<id5>/` (gen 30+)
//! or a packed `sound/<id5>.ifs` (gen < 30); both are consumed directly by `render_song`, which resolves the keysound
//! archive + chart inside. Omnimix-revived songs are searched under the omnimix sound dir first.

use crate::common::{MusicInfo, PackTask, version_folder_name};

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

        match locate_input(&roots, &str_id5) {
            Some(input_path) => vec_task.push(PackTask {
                info: info.clone(),
                input_path,
                dst_path: build_dst(path_out, info.version, &info.str_title),
            }),
            None => eprintln!("[skip] #{} {}: song not found ({str_id5})", info.id, info.str_title),
        }
    }

    vec_task
}

// resolve a song's on-disk input under the given roots, trying each in order: a loose folder `<id5>/` (gen 30+) wins
// over a packed `<id5>.ifs` (gen < 30). render_song takes either form directly. None when neither exists in any root.
fn locate_input(roots: &[&Path], str_id5: &str) -> Option<PathBuf> {
    for root in roots {
        let path_dir = root.join(str_id5);
        if path_dir.is_dir() {
            return Some(path_dir);
        }
        let path_ifs = root.join(format!("{str_id5}.ifs"));
        if path_ifs.is_file() {
            return Some(path_ifs);
        }
    }
    None
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
