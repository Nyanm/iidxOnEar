//! Extract a song's full-mix audio out of its container, yielding the raw WMA or WAV bytes for the transcoder.
//!
//! Every container reduces to "find the largest block": an `.s3p` (or an S3P0 inside an `.ifs`) holds `S3V0` blocks
//! wrapping WMA, the biggest being the full mix; a `.2dx` / the 2DX9 sequence inside an old `.ifs` holds `2DX9` blocks
//! wrapping RIFF/WAVE, the biggest being the full mix. Two oddities are handled by fallback: an old 2dx-only `.ifs` can
//! contain a stray `S3P0` byte pattern (a failed s3p extraction then falls back to 2DX9), and some omni `.2dx` are a
//! bare RIFF/WAVE with no 2DX9 wrapper. We never parse the IFS/KBin tree — a magic scan suffices.

use crate::common::AudioSource;

use std::fs;
use anyhow::{Context, Result, bail};

// the extracted full-mix audio, tagged by codec so the transcoder picks the right ffmpeg input handling
pub enum AudioBlob {
    Wma(Vec<u8>),               // ASF/WMA bytes from an S3V0 block (s3p family)
    Wav(Vec<u8>),               // RIFF/WAVE bytes from a 2DX9 block (2dx family)
}

impl AudioBlob {
    // byte length of the contained audio, for diagnostics
    pub fn len(&self) -> usize {
        match self {
            AudioBlob::Wma(v) | AudioBlob::Wav(v) => v.len(),
        }
    }

    // raw audio bytes, to be written to the ffmpeg input temp file
    pub fn bytes(&self) -> &[u8] {
        match self {
            AudioBlob::Wma(v) | AudioBlob::Wav(v) => v,
        }
    }

    // file extension for the ffmpeg input temp file, so ffmpeg picks the right demuxer
    pub fn input_ext(&self) -> &'static str {
        match self {
            AudioBlob::Wma(_) => "wma",
            AudioBlob::Wav(_) => "wav",
        }
    }
}

// extract the full-mix audio from a song's resolved audio source
pub fn unpack(src: &AudioSource) -> Result<AudioBlob> {
    match src {
        AudioSource::Loose(path) => {
            let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("s3p")) {
                Ok(AudioBlob::Wma(extract_s3p_wma(&data, 0)?))
            } else {
                Ok(AudioBlob::Wav(extract_2dx_wav(&data)?))
            }
        }
        AudioSource::Ifs(path) => {
            let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            // v25+ ifs hold a real S3P0 (the first match); an old 2dx-ifs may contain a stray S3P0 pattern that passes
            // the header sanity check, so if the s3p extraction fails, fall back to the 2DX9 path.
            if let Some(off) = find_valid_s3p0(&data) {
                if let Ok(wma) = extract_s3p_wma(&data, off) {
                    return Ok(AudioBlob::Wma(wma));
                }
            }
            Ok(AudioBlob::Wav(extract_2dx_wav(&data)?))
        }
    }
}

// --- s3p (WMA) ----------------------------------------------------------------------------------------------------

// extract the largest S3V0 entry's WMA payload, where the S3P0 header sits at `base` within `data` (0 for a loose
// .s3p; the scan offset for one inside an .ifs). Entry table offsets are relative to `base`.
fn extract_s3p_wma(data: &[u8], base: usize) -> Result<Vec<u8>> {
    let s3p = data.get(base..).context("s3p base out of range")?;
    let count = rd_u32(s3p, 0x04).context("s3p header truncated")? as usize;

    // pick the largest (offset, size) entry: the pre-mixed full song, vs the many small keysounds
    let mut best: Option<(usize, usize)> = None;
    for i in 0..count {
        let off = rd_u32(s3p, 0x08 + i * 8).context("s3p table truncated")? as usize;
        let size = rd_u32(s3p, 0x08 + i * 8 + 4).context("s3p table truncated")? as usize;
        if best.is_none_or(|(_, best_size)| size > best_size) {
            best = Some((off, size));
        }
    }
    let (off, size) = best.context("s3p has no entries")?;

    let block = s3p.get(off..off + size).context("s3p entry out of range")?;
    if block.get(0..4) != Some(b"S3V0".as_slice()) {
        bail!("s3p entry is not an S3V0 block");
    }
    let header_size = rd_u32(block, 0x04).context("S3V0 header truncated")? as usize;
    let data_size = rd_u32(block, 0x08).context("S3V0 header truncated")? as usize;
    let wma = block.get(header_size..header_size + data_size).context("S3V0 payload out of range")?;
    Ok(wma.to_vec())
}

// locate the first plausible S3P0 in `data`: its count must be reasonable and its entry table must fit. This still
// admits a rare stray "S3P0" byte pattern, which is why the Ifs path falls back to 2DX9 if the s3p extraction fails.
fn find_valid_s3p0(data: &[u8]) -> Option<usize> {
    let mut pos = 0;
    while let Some(p) = find_from(data, b"S3P0", pos) {
        pos = p + 4;
        if let Some(count) = rd_u32(data, p + 0x04).map(|c| c as usize) {
            if (1..=100_000).contains(&count) && p + 0x08 + count * 8 <= data.len() {
                return Some(p);
            }
        }
    }
    None
}

// --- 2dx (WAV) ----------------------------------------------------------------------------------------------------

// extract the full-mix WAV from a 2dx-family container: the largest 2DX9 block, or — when there is no 2DX9 wrapper at
// all — the whole buffer if it is a bare RIFF/WAVE (some omni .2dx ship the WAV directly).
fn extract_2dx_wav(data: &[u8]) -> Result<Vec<u8>> {
    if let Some(wav) = largest_2dx9_block(data) {
        return Ok(wav);
    }
    if data.get(0..4) == Some(b"RIFF".as_slice()) {
        return Ok(data.to_vec());
    }
    bail!("no 2DX9 block found and not a bare RIFF/WAVE");
}

// scan every "2DX9" block and return the largest block's RIFF/WAVE payload, or None when none validate. Each candidate
// is validated (sizes in range, payload starts with "RIFF") so a stray magic in audio data is silently ignored.
fn largest_2dx9_block(data: &[u8]) -> Option<Vec<u8>> {
    let mut best: Option<(usize, usize)> = None;                        // (data offset, data size)
    let mut pos = 0;
    while let Some(p) = find_from(data, b"2DX9", pos) {
        pos = p + 4;
        if let Some((doff, size)) = block_2dx9(data, p) {
            if best.is_none_or(|(_, best_size)| size > best_size) {
                best = Some((doff, size));
            }
        }
    }
    let (doff, size) = best?;
    Some(data[doff..doff + size].to_vec())
}

// validate a 2DX9 block at `p`: returns (payload offset, payload size) when the sizes are in range and the payload is
// a RIFF/WAVE, else None (truncated header, out-of-range, or a stray "2DX9" pattern inside audio data)
fn block_2dx9(data: &[u8], p: usize) -> Option<(usize, usize)> {
    let header_size = rd_u32(data, p + 0x04)? as usize;
    let data_size = rd_u32(data, p + 0x08)? as usize;
    let doff = p.checked_add(header_size)?;
    let dend = doff.checked_add(data_size)?;
    (dend <= data.len() && data.get(doff..doff + 4) == Some(b"RIFF".as_slice())).then_some((doff, data_size))
}

// --- low-level helpers --------------------------------------------------------------------------------------------

// read a little-endian u32 at `off`, returning None when the slice is too short (offsets here come from scanned data)
fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

// first index >= `start` where `needle` occurs in `data`, or None; repeated calls advancing `start` stay O(n) overall
fn find_from(data: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    data.get(start..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| start + i)
}
