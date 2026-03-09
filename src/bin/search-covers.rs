use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use lru::LruCache;
use scc::HashSet as ConcurrentHashSet;
use std::collections::BTreeMap;
use std::num::NonZero;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};
use std::thread;
use triforce::concurrency::*;

use clap::Parser;
use triforce::cli::*;
use triforce::*;

/// Search for a fixed region F between A and B (A ⊆ F ⊆ B) such that all
/// extensions of F within C have a meta-graph where each connected component
/// contains a completion of the given partial tiling.
///
/// This command automatically adds the nodes from A to B before the search.
#[derive(Parser, Debug)]
#[command(name = "gen-covers", version)]
struct Cli {
    /// Nodes required to be in the fixed region
    #[arg(required = true)]
    a: GraphSource,

    /// Nodes allowed in the fixed region
    #[arg(required = true)]
    b: GraphSource,

    /// Nodes allowed in covers of the fixed region
    #[arg(required = true)]
    c: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,

    /// A partial tiling we are required to fill
    partial_tiling: Option<PathBuf>,

    /// Check cover counterexamples exactly
    #[arg(required = false, long = "exact-cover", default_value_t = true)]
    exact_cover_check: bool,
}

// Search for a "happy" fixed region using given cost function.
//
// The cost function is assumed to be additive, so the cost of a graph is the
// sum of the cost of its nodes.
//
// The search procedure uses a priority queue based on the lowest cost for its
// worklist.
fn search_happy_cover<F: Fn(Node) -> isize + Sync + Send>(
    base: Graph,
    extensions: &Graph,
    allowed_in_covers: &Graph,
    partial_tiling: &BTreeMap<Node, Color>,
    tile_size: usize,
    cost: F,
    exact_cover_check: bool,
) -> Option<Graph> {
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

    // We keep failed extensions around to quickly refute a particular region
    let counterexamples = RwLock::new(HashSet::<Region>::new());

    let n_threads = rayon::current_num_threads();
    let regions_tried = ConcurrentHashSet::new();
    let worklist = Worklist::new([WithCost(base, 0)], n_threads, &regions_tried);

    // The cache of good covers, to skip re-tiling the same cover
    let good_cover_cache = Mutex::new(LruCache::<CompactRegion, ()>::new(
        NonZero::new(100_000_000).unwrap(),
    ));

    let i = AtomicUsize::new(0);
    let solution = Mutex::new(None);

    let worker = || {
        loop {
            let WithCost(graph, curr_cost) = match worklist.pop() {
                Task::Done => return,
                Task::Todo(g) => g,
            };

            // Each result is a Result that is:
            // - Ok(graph) if the result is fine
            // - Err(Option(counterexample)) if a new counterexample is discovered
            let check_graph = || -> Result<Graph, Option<CompactRegion>> {
                i.fetch_add(1, Ordering::SeqCst);
                // check if this region is already refuted

                // might want to skip this?
                if !exact_cover_check
                    && counterexamples
                        .read()
                        .unwrap()
                        .iter()
                        .any(|cex| graph.nodes().iter().all(|n| cex.contains(n)))
                {
                    return Err(None);
                }

                let empty_set = HashSet::default();
                let covers = {
                    let cexs = counterexamples.read().unwrap();
                    Tiling::min_covers(
                        &graph,
                        allowed_in_covers,
                        tile_size,
                        if exact_cover_check { &cexs } else { &empty_set },
                    )
                };
                let Some(covers) = covers else {
                    return Err(None);
                };

                let done = worklist.is_done();
                println!(
                    "tried {:>5} graphs (including counterexample refutations), graphs in queue: {}, done: {done}",
                    i.load(Ordering::SeqCst),
                    worklist.len(),
                );
                if done {
                    // for early exit to skip tiling the last one
                    return Err(None);
                }

                // for cover in &covers {
                //     println!("{}", serde_json::to_string(&MaybeRegion::from_region(cover.clone())).unwrap());
                // }

                println!("graph: {:?}", graph.nodes());
                println!("#covers: {}", covers.len());
                assert_ne!(covers.len(), 0);

                // check if all covers have a desired metagraph

                let tilings_tried = AtomicUsize::new(0);
                let first_cex = covers.iter().find(|cover| {
                    // skip the cover if it is already checked and in cache
                    // this is an expensive operation w.r.t. multithreading
                    {
                        // cannot use `contains` because it does not update the LRU cache.
                        if good_cover_cache.lock().unwrap().get(cover).is_some() {
                            return false;
                        }
                    }

                    let region = cover.to_region(allowed_in_covers);

                    // if exact_cover_check && counterexamples.contains(&region) {
                    //     return Some(cover);
                    // }

                    let g = Graph::from(region);
                    let tilings = Tiling::enumerate(&g, tile_size);
                    let complete = tilings
                        .iter()
                        .filter(|g| g.is_complete())
                        .collect::<Vec<_>>();

                    tilings_tried.fetch_add(complete.len(), Ordering::SeqCst);

                    // these are the nodes reachable from a completion of the partial tiling
                    let mut seen = HashSet::<Tiling>::new();

                    // find and mark connected components
                    let mut success = false;
                    for tiling in &complete {
                        // check for early return
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
                            // this is a completion of the partial tiling, mark all reachable nodes as seen.
                            seen.extend(tiling.reachable(tile_size));
                        }
                    }

                    if seen.len() == complete.len() {
                        success = true;
                    }

                    if success {
                        // this is a good cover, add it to the cache
                        good_cover_cache.lock().unwrap().put(**cover, ());

                        false
                    } else {
                        println!("failing cover: {:?}", cover.to_region(allowed_in_covers));
                        true
                    }
                });

                println!("# tilings tried: {}", tilings_tried.load(Ordering::SeqCst));

                if first_cex.is_none() {
                    return Ok(graph.clone());
                }

                Err(first_cex.cloned())
            };

            let add_neighbors = || {
                let r = graph.clone().into_region();

                let neighbors = r
                    .neighbors()
                    .expect("the starting graph cannot be empty")
                    .into_iter()
                    .filter(|n| extensions.contains(n))
                    .collect::<Vec<_>>();

                let mut new_graphs = vec![];
                for n in neighbors {
                    let mut new = r.clone();
                    new.insert(n);
                    let new = Graph::from(new);
                    let cost = curr_cost + cost(n);
                    if !regions_tried.contains_sync(&new) {
                        new_graphs.push(WithCost(new, cost));
                    }
                }
                worklist.push_all(new_graphs);
            };

            match check_graph() {
                Ok(graph) => {
                    let mut s = solution.lock().unwrap();
                    println!("FOUND {:?}", graph.nodes());

                    if s.is_none() {
                        *s = Some(graph);
                    }
                    // signal that we're done
                    worklist.done();

                    return;
                }
                Err(cex) => {
                    add_neighbors();

                    if let Some(cex) = cex {
                        counterexamples
                            .write()
                            .unwrap()
                            .insert(cex.to_region(allowed_in_covers));
                    }
                }
            }
        }
    };

    // start the thread pool
    thread::scope(|s| {
        for _ in 0..n_threads {
            s.spawn(worker);
        }
    });

    solution.lock().unwrap().take()
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let base = read_graph(cli.a, false);
    let mut extensions_r = read_graph(cli.b, false).into_region();
    extensions_r.append(&mut base.clone().into_region());
    let mut allowed_in_covers_r = read_graph(cli.c, true).into_region();
    allowed_in_covers_r.append(&mut extensions_r.clone());
    let extensions = Graph::from(extensions_r);
    let allowed_in_covers = Graph::from(allowed_in_covers_r);

    let partial_tiling = cli.partial_tiling.map_or(BTreeMap::new(), read_tiling);

    let k = cli.tile_size;

    let cost = |n: Node| {
        partial_tiling
            .keys()
            .map(|s| {
                let dx = (n.0 - s.0).abs() as isize;
                let dy = (n.1 - s.1).abs() as isize;
                dx + 0.max(dy - dx)
            })
            .max()
            .unwrap()
    };

    assert!(
        extensions.len() <= BYTES_IN_COMPACT_REGION * 8,
        "{} > {}",
        extensions.len(),
        BYTES_IN_COMPACT_REGION * 8
    );

    println!("exact cover check: {}", cli.exact_cover_check);

    match search_happy_cover(
        base,
        &extensions,
        &allowed_in_covers,
        &partial_tiling,
        k,
        cost,
        cli.exact_cover_check,
    ) {
        None => {
            println!("No suitable region is found");
        }
        Some(r) => {
            println!("found {}", serde_json::to_string(r.nodes()).unwrap());
        }
    }
}
