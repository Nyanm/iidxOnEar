//! Album cover art: map each game version to its KONAMI mobile-site album jacket URL, download missing ones into the
//! local jacket cache, and resolve per-version paths for the packager to embed.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use anyhow::{Context, Result};

// full source URL per version (index == version); empty == no cover. Only non-empty entries are fetched/checked.
const VERSION_JACKET_URL: [&str; 41] = [
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01000.jpg", // 0  1st style era
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01000.jpg", // 1  Substream -> 1st
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01001.jpg", // 2  2nd style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01002.jpg", // 3  3rd style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01003.jpg", // 4  4th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01004.jpg", // 5  5th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01005.jpg", // 6  6th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01006.jpg", // 7  7th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01007.jpg", // 8  8th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01008.jpg", // 9  9th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01009.jpg", // 10 10th style
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01010.jpg", // 11 IIDX RED
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01011.jpg", // 12 HAPPY SKY
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01012.jpg", // 13 DistorteD
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01013.jpg", // 14 GOLD
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01014.jpg", // 15 DJ TROOPERS
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01015.jpg", // 16 EMPRESS
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01016.jpg", // 17 SIRIUS
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01018.jpg", // 18 Resort Anthem
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01019.jpg", // 19 Lincle
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01020.jpg", // 20 tricoro
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01021.jpg", // 21 SPADA (disc1)
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01023.jpg", // 22 PENDUAL (disc1)
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01025.jpg", // 23 copula (disc1)
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01027.jpg", // 24 SINOBUZ
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01028.jpg", // 25 CANNON BALLERS
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01029.jpg", // 26 Rootage
    "https://media.vgm.io/albums/94/95249/95249-4674da022712.jpg",     // 27 HEROIC VERSE (vgmdb OST; eacache is key-visual)
    "https://media.vgm.io/albums/06/107560/107560-13529c73082d.jpg",   // 28 BISTROVER (vgmdb OST)
    "https://media.vgm.io/albums/31/117313/117313-b11b29630afd.jpg",   // 29 CastHour (vgmdb OST)
    "https://media.vgm.io/albums/15/125751/125751-d316c581c5dd.jpg",   // 30 RESIDENT (vgmdb OST)
    "https://media.vgm.io/albums/74/135147/135147-2de459935c8d.jpg",   // 31 EPOLIS (vgmdb OST)
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01037.jpg", // 32 Pinky Crush (key-visual; OST not released)
    "https://eacache.s.konaminet.jp/game/2dx/mobile/images/music/jk/albumjacket_01039.jpg", // 33 Sparkle Shower (key-visual; OST not released)
    "", // 34  placeholder: no cover yet
    "", // 35
    "", // 36
    "", // 37
    "", // 38
    "", // 39
    "", // 40
];

// browsers' UA; the CDN rejects requests without one. Overall per-request timeout for a hung connection.
const USER_AGENT: &str = "Mozilla/5.0";
const TIMEOUT_SECS: u64 = 25;

// local cache filename for a version's jacket: "<version>.<ext>", ext taken from the source URL (jpg/png/...)
fn local_name(version: u8, url: &str) -> String {
    let ext = url.rsplit('.').next().unwrap_or("jpg");
    format!("{version}.{ext}")
}

// resolve the local jacket path for a version (whether or not it exists yet); None when the version has no source URL
pub fn jacket_path(jacket_dir: &Path, version: u8) -> Option<PathBuf> {
    let url = *VERSION_JACKET_URL.get(version as usize)?;
    (!url.is_empty()).then(|| jacket_dir.join(local_name(version, url)))
}

// ensure every version's jacket is cached in `jacket_dir`, downloading missing ones; failures are non-fatal
pub fn ensure_jackets(jacket_dir: &Path) -> Result<()> {
    fs::create_dir_all(jacket_dir).with_context(|| format!("creating jacket dir {}", jacket_dir.display()))?;

    let (mut count_ok, mut count_have, mut count_fail) = (0u32, 0u32, 0u32);
    for (version, &url) in VERSION_JACKET_URL.iter().enumerate() {
        if url.is_empty() {
            continue;
        }
        let path = jacket_dir.join(local_name(version as u8, url));
        if path.exists() {
            count_have += 1;
            continue;
        }
        match download(url, &path) {
            Ok(()) => count_ok += 1,
            Err(e) => {
                count_fail += 1;
                eprintln!("jacket: FAILED v{version} ({url}): {e:#}");
            }
        }
    }
    println!("jackets: {count_ok} downloaded, {count_have} cached, {count_fail} failed -> {}", jacket_dir.display());
    Ok(())
}

// download `url` to `path` via a temp file + atomic rename; blocking, with an overall per-request timeout
fn download(url: &str, path: &Path) -> Result<()> {
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .set("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("requesting {url}"))?;

    let mut vec_bytes = Vec::new();
    resp.into_reader().read_to_end(&mut vec_bytes).context("reading response body")?;

    let path_tmp = path.with_extension("part");
    fs::write(&path_tmp, &vec_bytes).with_context(|| format!("writing {}", path_tmp.display()))?;
    fs::rename(&path_tmp, path).with_context(|| format!("finalizing {}", path.display()))?;
    Ok(())
}
