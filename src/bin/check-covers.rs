use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
        println!(
            "TIMER {}: {:.3}ms",
            msg,
            self.start.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn restart(&mut self) {
        self.start = Instant::now();
    }
}

/// Check whether the fixed region A has a meta-graph where each connected
/// component contains a completion of the given partial tiling, for all
/// extensions of A within B.
///
/// This is equivalent to search-covers with A == B (i.e., the fixed region
/// is exactly A, with no room to grow).
#[derive(Parser, Debug)]
#[command(name = "check-covers", version)]
struct Cli {
    /// The fixed region (nodes required in and allowed in the fixed region)
    #[arg(required = true)]
    a: GraphSource,

    /// Nodes allowed in covers of the fixed region
    #[arg(required = true)]
    b: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,

    /// A partial tiling we are required to fill
    partial_tiling: Option<PathBuf>,
}

fn check_happy_cover(
    base: &Graph,
    allowed_in_covers: &Graph,
    partial_tiling: &BTreeMap<Node, Color>,
    tile_size: usize,
) -> bool {
    let mut timer = Timer::new();

    let partial_tile_set = {
        let mut color2tile = HashMap::<Color, Vec<Node>>::new();
        for (node, color) in partial_tiling.iter() {
            color2tile.entry(*color).or_default().push(*node);
        }

        // Verify that the partial tiling uses tiles of the same size as tile_size
        assert!(
            color2tile.values().all(|ns| ns.len() == tile_size),
            "the partial tiling should use tiles of size {tile_size}"
        );

        color2tile.into_values().collect::<HashSet<Vec<Node>>>()
    };

    timer.record_elapsed("partial_tile_set");
    timer.restart();

    let covers = Tiling::par_min_covers(base, allowed_in_covers, tile_size);

    timer.record_elapsed("min_covers");

    let Some(covers) = covers else {
        return false;
    };

    println!("graph: {:?}", base.nodes());
    println!("#covers: {}", covers.len());
    assert_ne!(covers.len(), 0);

    let mut tilings_tried = 0usize;

    // Accumulators for per-cover sub-step timings
    let mut t_to_region = Duration::ZERO;
    let mut t_enumerate = Duration::ZERO;
    let mut t_filter = Duration::ZERO;
    let mut t_reachable = Duration::ZERO;

    let covers_loop_start = Instant::now();
    let all_pass = covers.iter().all(|cover| {
        timer.restart();
        let region = cover.to_region(allowed_in_covers);
        let g = Graph::from(region);
        t_to_region += timer.start.elapsed();

        timer.restart();
        let tilings = Tiling::enumerate(&g, tile_size);
        t_enumerate += timer.start.elapsed();

        timer.restart();
        let complete = tilings
            .iter()
            .filter(|g| g.is_complete())
            .collect::<Vec<_>>();
        t_filter += timer.start.elapsed();

        tilings_tried += complete.len();

        timer.restart();
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
        t_reachable += timer.start.elapsed();

        if !success {
            println!("failing cover: {:?}", cover.to_region(allowed_in_covers));
        }

        success
    });

    println!("# tilings tried: {tilings_tried}");

    println!(
        "TIMER covers_loop/total: {:.3}ms",
        covers_loop_start.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "TIMER covers_loop/to_region+graph: {:.3}ms",
        t_to_region.as_secs_f64() * 1000.0
    );
    println!(
        "TIMER covers_loop/enumerate: {:.3}ms",
        t_enumerate.as_secs_f64() * 1000.0
    );
    println!(
        "TIMER covers_loop/filter_complete: {:.3}ms",
        t_filter.as_secs_f64() * 1000.0
    );
    println!(
        "TIMER covers_loop/reachable: {:.3}ms",
        t_reachable.as_secs_f64() * 1000.0
    );

    all_pass
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let base = read_graph(cli.a, false);
    let mut allowed_in_covers_r = read_graph(cli.b, true).into_region();
    allowed_in_covers_r.append(&mut base.clone().into_region());
    let allowed_in_covers = Graph::from(allowed_in_covers_r);

    let partial_tiling = cli.partial_tiling.map_or(BTreeMap::new(), read_tiling);

    let k = cli.tile_size;

    assert!(
        base.len() <= BYTES_IN_COMPACT_REGION * 8,
        "{} > {}",
        base.len(),
        BYTES_IN_COMPACT_REGION * 8
    );

    if check_happy_cover(&base, &allowed_in_covers, &partial_tiling, k) {
        println!("found {}", serde_json::to_string(base.nodes()).unwrap());
    } else {
        println!("No suitable region is found");
    }
}
