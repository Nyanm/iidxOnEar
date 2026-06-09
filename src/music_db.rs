//! Load `music_data.bin` (IIDX 27+ wide-string format) into a dense, song-id-indexed `Vec<MusicInfo>`.
//!
//! Layout (little-endian): a 16-byte header, then an `int32[index_len]` table keyed by song_id (value = entry number,
//! -1 = id absent), then `count` fixed-size entries. We iterate the `count` entries and place each at its own song_id
//! (read from the entry), so the returned vector is keyed by song_id (the on-disk 5-digit name), gaps left as default.

use crate::common::MusicInfo;

use std::fs;
use std::path::Path;
use anyhow::{Context, Result, bail};

// IIDX 32 era entry size, reverse-engineered as (file_size - data_start) / count; we validate it against the file
const ENTRY_SIZE: usize = 2040;

// public entry: load and index the whole music database, returning a vector indexed by song_id
pub fn load_index(path_bin: &Path) -> Result<Vec<MusicInfo>> {
    let data = fs::read(path_bin).with_context(|| format!("reading music_data.bin {}", path_bin.display()))?;
    parse_index(&data)
}

// --- parse: binary bytes -> dense song-id-indexed vector ----------------------------------------------------------

// parse the whole buffer into a vector whose index equals song_id; gaps are default (is_valid == false) entries
fn parse_index(data: &[u8]) -> Result<Vec<MusicInfo>> {
    if data.len() < 0x10 || &data[0..4] != b"IIDX" {
        bail!("not a music_data.bin (bad magic)");
    }
    let _version = rd_u32(data, 0x04);
    let count = rd_u32(data, 0x08) as usize;
    let index_len = rd_u32(data, 0x0C) as usize;
    let data_start = 0x10 + index_len * 4;

    let body = data.len().checked_sub(data_start).filter(|_| count > 0).context("file smaller than header")?;
    if body % count != 0 || body / count != ENTRY_SIZE {
        bail!("unexpected entry size {} (expected {ENTRY_SIZE}); is this the v32 music_data.bin?", body.checked_div(count).unwrap_or(0));
    }

    // vector keyed by song_id; index_len is the song_id upper bound, so every real id fits (resized defensively below)
    let mut vec_music: Vec<MusicInfo> = vec![MusicInfo::default(); index_len];
    for n in 0..count {
        let off = data_start + n * ENTRY_SIZE;
        let e = &data[off..off + ENTRY_SIZE];
        let song_id = rd_u32(e, 0x67C) as usize;                        // the entry's own canonical id
        if song_id >= vec_music.len() {
            vec_music.resize(song_id + 1, MusicInfo::default());        // defensive: id beyond index_len
        }
        vec_music[song_id] = parse_entry(song_id as u32, e);
    }
    Ok(vec_music)
}

// --- omnimix merge ------------------------------------------------------------------------------------------------

// fold an omnimix index into the base one: each valid omni song fills a gap in `vec_base` and is flagged is_omnimix.
// existing valid base entries are never overwritten. returns how many were added.
pub fn merge_omnimix(vec_base: &mut Vec<MusicInfo>, vec_omni: Vec<MusicInfo>) -> usize {
    let mut count_added = 0;
    for mut info in vec_omni {
        if !info.is_valid {
            continue;
        }
        let idx = info.id as usize;
        if idx >= vec_base.len() {
            vec_base.resize(idx + 1, MusicInfo::default());
        }
        if !vec_base[idx].is_valid {
            info.is_omnimix = true;
            vec_base[idx] = info;
            count_added += 1;
        }
    }
    count_added
}

// read one fixed-size entry into a MusicInfo; `song_id` is the entry's own id (0x67C), the canonical key
fn parse_entry(song_id: u32, e: &[u8]) -> MusicInfo {
    MusicInfo {
        is_valid: true,
        is_omnimix: false,
        id: song_id,
        str_title: wstr(e, 0x000, 0x100),
        str_genre: wstr(e, 0x140, 0x80),
        str_artist: wstr(e, 0x1C0, 0x100),
        version: rd_u16(e, 0x3DC) as u8,
    }
}

// --- low-level field readers --------------------------------------------------------------------------------------

// read a little-endian u32 at `off` (callers guarantee the slice is long enough)
fn rd_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

// read a little-endian u16 at `off`
fn rd_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

// decode a UTF-16LE field of `size` bytes at `off`, stopping at the first NUL
fn wstr(b: &[u8], off: usize, size: usize) -> String {
    let units: Vec<u16> = b[off..off + size]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}
