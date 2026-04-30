use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::Parser;
use triforce::cli::*;
use triforce::*;

struct Timer {
    start: Instant,
}

impl Timer {
    fn new() -> Self {
        Timer {
            start: Instant::now(),
        }
    }

    fn record_elapsed(&self, msg: &str) {
        log::info!(
            "TIMER {}: {:.3}ms",
            msg,
            self.start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/// Check a slice of pre-generated potential covers.
///
/// Reads a JSON covers file produced by dump-potential-covers and checks only
/// covers[range_start..range_end] against the given partial tiling. This allows
/// the full check to be distributed across multiple machines.
#[derive(Parser, Debug)]
#[command(name = "check-given-covers", version)]
struct Cli {
    /// JSON file of potential covers (output of dump-potential-covers)
    covers: PathBuf,

    /// Size of the tiles to tile the graph with
    tile_size: usize,

    /// A partial tiling we are required to fill
    partial_tiling: PathBuf,

    /// Start of the range to check (inclusive)
    range_start: usize,

    /// End of the range to check (exclusive)
    range_end: usize,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    let all_covers: Vec<MaybeRegion> =
        serde_json::from_slice(&std::fs::read(&cli.covers).expect("could not read covers file"))
            .expect("covers file is not a valid JSON array of regions");

    let covers = &all_covers[cli.range_start..cli.range_end];

    let partial_tiling = read_tiling(cli.partial_tiling);
    let tile_size = cli.tile_size;

    let partial_tile_set = {
        let mut color2tile = HashMap::<Color, Vec<Node>>::new();
        for (node, color) in partial_tiling.iter() {
            color2tile.entry(*color).or_default().push(*node);
        }
        assert!(
            color2tile.values().all(|ns| ns.len() == tile_size),
            "the partial tiling should use tiles of size {tile_size}"
        );
        color2tile.into_values().collect::<HashSet<Vec<Node>>>()
    };

    let timer = Timer::new();
    let tilings_tried = AtomicUsize::new(0);

    let all_pass = covers
        .iter()
        .enumerate()
        .par_bridge()
        .map(|(i, maybe_region)| {
            log::info!("starting cover {i} (offset from start)");

            let region = maybe_region
                .clone()
                .to_region(false)
                .expect("cover in JSON file is not a valid region");

            let g = Graph::from(region.clone());
            let tilings = Tiling::enumerate(&g, tile_size);
            let complete = tilings
                .into_iter()
                .filter(|t| t.is_complete())
                .collect::<Vec<_>>();

            if complete.is_empty() {
                log::warn!("The metagraph for {g:?} is empty!");
                return true;
            }

            tilings_tried.fetch_add(complete.len(), Ordering::SeqCst);

            let mut seen = HashSet::<Tiling>::new();
            let mut success = false;

            for tiling in &complete {
                if seen.len() == complete.len() {
                    success = true;
                    break;
                }

                if seen.contains(tiling) {
                    continue;
                }

                if partial_tile_set.iter().all(|tile| {
                    let color = tiling.color(&tile[0]);
                    tile.iter().all(|n| tiling.color(n) == color)
                }) {
                    seen.extend(tiling.reachable(tile_size));
                }
            }

            if seen.len() == complete.len() {
                success = true;
            }

            println!("finished cover {i} (offset from start)");
            if !success {
                println!("failing cover: {:?}", region);
            }

            success
        })
        .reduce(|| true, |a, b| a && b);

    timer.record_elapsed("metagraph_check");

    println!("# tilings tried: {}", tilings_tried.load(Ordering::SeqCst));

    if all_pass {
        println!("all pass");
    } else {
        println!("some covers failed");
        std::process::exit(1);
    }
}
