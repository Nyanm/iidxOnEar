//! Shared data definitions: the parsed song record, the packaging task, and the per-version name tables.

use std::path::PathBuf;

// one song entry; index in the loaded vector equals `id` (the song_id), gaps are left with `is_valid == false`
#[derive(Default, Clone, Debug)]
pub struct MusicInfo {
    pub is_valid: bool,         // false for id gaps / placeholders, skipped on traversal
    pub is_omnimix: bool,       // revived by an omnimix patch -> audio lives under the omnimix sound dir
    pub id: u32,                // song_id, also the 5-digit on-disk folder/file name
    pub str_title: String,      // -> TITLE
    pub str_genre: String,      // -> GENRE
    pub str_artist: String,     // -> ARTIST
    pub version: u8,            // game version 0..=32, picks the album/output folder (0 == 1st style era)
}

// one Opus file to produce: the song's on-disk input, the metadata, and the output path
#[derive(Debug, Clone)]
pub struct PackTask {
    pub info: MusicInfo,        // song metadata, source of the Vorbis tags
    pub input_path: PathBuf,    // the song folder (v30+) or "<id5>.ifs" (v1-29) handed to iidx_on_knitting::render_song
    pub dst_path: PathBuf,      // output ".opus" path
}

// fixed ALBUMARTIST tag so each version-album groups correctly despite differing per-track artists
pub const ALBUM_ARTIST: &str = "BEMANI";

// ALBUM tag per version: the full official game title; index == version (slot 0 == 1st style for version-0 songs)
pub const VERSION_ALBUM_NAMES: [&str; 41] = [
    "beatmania IIDX 1st style",         // 0  (version-0 songs are 1st style era)
    "beatmania IIDX Substream",         // 1
    "beatmania IIDX 2nd style",         // 2
    "beatmania IIDX 3rd style",         // 3
    "beatmania IIDX 4th style",         // 4
    "beatmania IIDX 5th style",         // 5
    "beatmania IIDX 6th style",         // 6
    "beatmania IIDX 7th style",         // 7
    "beatmania IIDX 8th style",         // 8
    "beatmania IIDX 9th style",         // 9
    "beatmania IIDX 10th style",        // 10
    "beatmania IIDX 11 IIDX RED",       // 11
    "beatmania IIDX 12 HAPPY SKY",      // 12
    "beatmania IIDX 13 DistorteD",      // 13
    "beatmania IIDX 14 GOLD",           // 14
    "beatmania IIDX 15 DJ TROOPERS",    // 15
    "beatmania IIDX 16 EMPRESS",        // 16
    "beatmania IIDX 17 SIRIUS",         // 17
    "beatmania IIDX 18 Resort Anthem",  // 18
    "beatmania IIDX 19 Lincle",         // 19
    "beatmania IIDX 20 tricoro",        // 20
    "beatmania IIDX 21 SPADA",          // 21
    "beatmania IIDX 22 PENDUAL",        // 22
    "beatmania IIDX 23 copula",         // 23
    "beatmania IIDX 24 SINOBUZ",        // 24
    "beatmania IIDX 25 CANNON BALLERS", // 25
    "beatmania IIDX 26 Rootage",        // 26
    "beatmania IIDX 27 HEROIC VERSE",   // 27
    "beatmania IIDX 28 BISTROVER",      // 28
    "beatmania IIDX 29 CastHour",       // 29
    "beatmania IIDX 30 RESIDENT",       // 30
    "beatmania IIDX 31 EPOLIS",         // 31
    "beatmania IIDX 32 Pinky Crush",    // 32
    "beatmania IIDX 33 Sparkle Shower", // 33
    "beatmania IIDX 34",                // 34  placeholder: future versions, number only (unnamed)
    "beatmania IIDX 35",                // 35
    "beatmania IIDX 36",                // 36
    "beatmania IIDX 37",                // 37
    "beatmania IIDX 38",                // 38
    "beatmania IIDX 39",                // 39
    "beatmania IIDX 40",                // 40
];

// output sub-folder per version, "IIDX NN <name>" with a zero-padded number for on-disk sorting; index == version
pub const VERSION_FOLDER_NAMES: [&str; 41] = [
    "IIDX 01 1st style",                // 0
    "IIDX 01 Substream",                // 1
    "IIDX 02 2nd style",                // 2
    "IIDX 03 3rd style",                // 3
    "IIDX 04 4th style",                // 4
    "IIDX 05 5th style",                // 5
    "IIDX 06 6th style",                // 6
    "IIDX 07 7th style",                // 7
    "IIDX 08 8th style",                // 8
    "IIDX 09 9th style",                // 9
    "IIDX 10 10th style",               // 10
    "IIDX 11 IIDX RED",                 // 11
    "IIDX 12 HAPPY SKY",                // 12
    "IIDX 13 DistorteD",                // 13
    "IIDX 14 GOLD",                     // 14
    "IIDX 15 DJ TROOPERS",              // 15
    "IIDX 16 EMPRESS",                  // 16
    "IIDX 17 SIRIUS",                   // 17
    "IIDX 18 Resort Anthem",            // 18
    "IIDX 19 Lincle",                   // 19
    "IIDX 20 tricoro",                  // 20
    "IIDX 21 SPADA",                    // 21
    "IIDX 22 PENDUAL",                  // 22
    "IIDX 23 copula",                   // 23
    "IIDX 24 SINOBUZ",                  // 24
    "IIDX 25 CANNON BALLERS",           // 25
    "IIDX 26 Rootage",                  // 26
    "IIDX 27 HEROIC VERSE",             // 27
    "IIDX 28 BISTROVER",                // 28
    "IIDX 29 CastHour",                 // 29
    "IIDX 30 RESIDENT",                 // 30
    "IIDX 31 EPOLIS",                   // 31
    "IIDX 32 Pinky Crush",              // 32
    "IIDX 33 Sparkle Shower",           // 33
    "IIDX 34",                          // 34  placeholder: future versions, number only (unnamed)
    "IIDX 35",                          // 35
    "IIDX 36",                          // 36
    "IIDX 37",                          // 37
    "IIDX 38",                          // 38
    "IIDX 39",                          // 39
    "IIDX 40",                          // 40
];

// album name for the ALBUM tag; out-of-range versions fall back to an empty string instead of panicking
pub fn version_album_name(version: u8) -> &'static str {
    VERSION_ALBUM_NAMES.get(version as usize).copied().unwrap_or("")
}

// output sub-folder name for the version; out-of-range falls back to an empty string
pub fn version_folder_name(version: u8) -> &'static str {
    VERSION_FOLDER_NAMES.get(version as usize).copied().unwrap_or("")
}
