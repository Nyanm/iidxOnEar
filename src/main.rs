//! IidxOnEar entry point: load the DB (+omnimix), plan one task per selected song, then render+tag each to Opus.

use music_db::{load_index, merge_omnimix};
use packer::package;
use scan::{filter_existing, find_omnimix, scan_songs};
use tool::dump_music_csv;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use anyhow::{Context, Result, bail};
use clap::Parser;

mod common;
mod jacket;
mod music_db;
mod packer;
mod scan;
mod tool;

// command-line arguments
#[derive(Parser)]
#[command(name = "IidxOnEar", about = "Extract beatmania IIDX songs into a tagged music library")]
struct Cli {
    /// IIDX `contents` directory (data/info/0/music_data.bin is appended automatically)
    #[arg(short, long)]
    src: PathBuf,

    /// output directory [default: current directory]
    #[arg(short, long)]
    dst: Option<PathBuf>,

    /// only convert these game versions, e.g. -v 30 31 32 [default: all versions]
    #[arg(short = 'v', long = "version", num_args = 1.., value_name = "VER")]
    versions: Vec<u8>,

    /// re-convert songs even if the output already exists (default: skip existing, incremental)
    #[arg(short, long)]
    force: bool,

    /// number of parallel workers [default: logical CPU count]
    #[arg(short, long)]
    jobs: Option<usize>,

    /// dump the parsed database to ./music_data.csv and exit (for auditing the decode)
    #[arg(long)]
    csv: bool,

    /// download all album jackets into ./jacket and exit (no DB/scan/transcode)
    #[arg(long)]
    test_jacket: bool,

    /// skip the album-jacket step entirely (no download, no embedded cover) — for dead URLs or a poor connection
    #[arg(long)]
    skip_jacket: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // album-jacket cache lives in ./jacket (cwd), reused across runs
    let path_jacket = std::env::current_dir().context("resolving current directory")?.join("jacket");

    // --test-jacket: just fetch all album jackets into ./jacket and exit (independent of the song DB)
    if cli.test_jacket {
        jacket::ensure_jackets(&path_jacket)?;
        return Ok(());
    }

    // load the base database from <src>/data/info/0/music_data.bin
    let path_bin = cli.src.join("data").join("info").join("0").join("music_data.bin");
    if !path_bin.is_file() {
        bail!("music_data.bin not found: {} (is --src pointing at the IIDX 'contents' directory?)", path_bin.display());
    }
    let mut vec_music = load_index(&path_bin)?;

    // fold in an omnimix patch if installed (revived deleted songs), searched up to 2 levels under the contents dir
    let path_omni = find_omnimix(&cli.src);
    if let Some(omni) = &path_omni {
        let vec_omni = load_index(&omni.join("info").join("0").join("music_omni.bin"))?;
        let count_omni = merge_omnimix(&mut vec_music, vec_omni);
        println!("omnimix: +{count_omni} revived songs from {}", omni.display());
    }
    println!("db: {} valid songs", vec_music.iter().filter(|m| m.is_valid).count());

    // --csv: dump the merged db to ./music_data.csv for manual decode auditing, then exit (no scan)
    if cli.csv {
        let path_csv = std::env::current_dir().context("resolving current directory")?.join("music_data.csv");
        dump_music_csv(&vec_music, &path_csv)?;
        println!("wrote {}", path_csv.display());
        return Ok(());
    }

    // resolve sound dirs + output dir, then plan one task per selected song
    let path_sound = cli.src.join("data").join("sound");
    if !path_sound.is_dir() {
        bail!("sound folder not found: {} (is --src pointing at the IIDX 'contents' directory?)", path_sound.display());
    }
    let path_sound_omni = path_omni.as_ref().map(|o| o.join("sound"));
    let path_out = match cli.dst {
        Some(path) => path,
        None => std::env::current_dir().context("resolving current directory for output")?,
    };

    let mut vec_task = scan_songs(&path_sound, path_sound_omni.as_deref(), &path_out, &vec_music, &cli.versions);
    print_plan(&vec_task, &path_out);

    // fetch album jackets (unless skipped); drop already-converted songs unless --force; then run the worker pool
    if !cli.skip_jacket {
        jacket::ensure_jackets(&path_jacket)?;
    }
    let count_planned = vec_task.len();
    if !cli.force {
        filter_existing(&mut vec_task);
    }
    let jobs = cli.jobs.unwrap_or_else(|| thread::available_parallelism().map_or(1, |n| n.get())).max(1);
    println!("converting {} tracks with {jobs} workers ({} already present)", vec_task.len(), count_planned - vec_task.len());

    // jacket dir for embedding; None when --skip-jacket so no cover is attached even if cached images exist
    let jacket_dir = (!cli.skip_jacket).then_some(path_jacket.as_path());
    convert_all(&vec_task, jobs, jacket_dir);

    Ok(())
}

// convert all tasks with a fixed pool of `jobs` workers; each atomically claims the next index until the list drains
fn convert_all(vec_task: &[common::PackTask], jobs: usize, jacket_dir: Option<&Path>) {
    let count_total = vec_task.len();
    let idx_next = AtomicUsize::new(0);                                 // next task index to hand out
    let count_done = AtomicUsize::new(0);                               // finished (ok or fail), for progress
    let count_fail = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| loop {
                let idx = idx_next.fetch_add(1, Ordering::Relaxed);
                if idx >= count_total {
                    break;
                }
                let task = &vec_task[idx];
                if let Err(e) = convert_one(task, jacket_dir) {
                    count_fail.fetch_add(1, Ordering::Relaxed);
                    eprintln!("FAIL #{} {}: {e:#}", task.info.id, task.info.str_title);
                }
                let count = count_done.fetch_add(1, Ordering::Relaxed) + 1;
                if count % 100 == 0 || count == count_total {
                    println!("  {count}/{count_total}");
                }
            });
        }
    });
    let count_fail = count_fail.load(Ordering::Relaxed);
    println!("done: {} converted, {count_fail} failed", count_total - count_fail);
}

// render one song (via iidx_on_knitting) and package it into the tagged opus, embedding the version's cached jacket
fn convert_one(task: &common::PackTask, jacket_dir: Option<&Path>) -> Result<()> {
    let jacket = jacket_dir
        .and_then(|dir| jacket::jacket_path(dir, task.info.version))
        .filter(|path| path.exists());
    package(&task.info, &task.input_path, jacket.as_deref(), &task.dst_path)
}

// print the planned task count overall and broken down per version
fn print_plan(vec_task: &[common::PackTask], path_out: &Path) {
    let mut map_per_version: BTreeMap<u8, usize> = BTreeMap::new();
    for task in vec_task {
        *map_per_version.entry(task.info.version).or_default() += 1;
    }
    println!("planned {} tracks -> {}", vec_task.len(), path_out.display());
    for (version, count) in map_per_version {
        println!("  v{version:<2} {:<28} {count}", common::version_folder_name(version));
    }
}
