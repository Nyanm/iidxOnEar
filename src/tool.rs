//! Diagnostic helpers, kept out of the normal conversion path.
//!
//! `dump_music_csv` writes the parsed database to a UTF-8 (BOM) CSV so the decode can be audited against the known-good
//! Python parser output, before any audio conversion is wired up.

use crate::common::MusicInfo;

use std::fs;
use std::path::Path;
use anyhow::{Context, Result};

// write every valid song's fields to a UTF-8 CSV with a BOM (so Excel reads CJK correctly), sorted by song_id
pub fn dump_music_csv(vec_music: &[MusicInfo], path_csv: &Path) -> Result<()> {
    let mut str_out = String::from("\u{FEFF}");                         // UTF-8 BOM for Excel
    str_out.push_str("id,omnimix,title,genre,artist,version\n");
    for info in vec_music.iter().filter(|m| m.is_valid) {
        str_out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            info.id,
            info.is_omnimix as u8,
            csv_field(&info.str_title),
            csv_field(&info.str_genre),
            csv_field(&info.str_artist),
            info.version,
        ));
    }
    fs::write(path_csv, str_out).with_context(|| format!("writing csv {}", path_csv.display()))?;
    Ok(())
}

// quote a field per RFC 4180 when it contains a comma, quote, or newline (titles/artists often have commas/parens)
fn csv_field(str_value: &str) -> String {
    if str_value.contains(|c| matches!(c, ',' | '"' | '\n' | '\r')) {
        format!("\"{}\"", str_value.replace('"', "\"\""))
    } else {
        str_value.to_string()
    }
}
