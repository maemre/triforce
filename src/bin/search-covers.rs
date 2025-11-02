use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use lru::LruCache;
use rayon::prelude::*;
use scc::HashSet as ConcurrentHashSet;
use std::collections::{BTreeMap, BinaryHeap};
use std::num::NonZero;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use triforce::cli::*;
use triforce::*;

/// Search for a fixed region F between A and B (A ⊆ F ⊆ B) such that all extensions
/// of F within C:
/// 1. have a connected meta-graph.
/// 2. can complete the given partial tiling.
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
    #[arg(required = false, long = "exact-cover", default_value_t = false)]
    exact_cover_check: bool,
}

/// A piece of data with associated cost.
///
/// `WithCost` objects are ordered according to their cost, so a <= b iff a.cost
/// >= b.cost.
#[derive(PartialEq, Eq, Ord)]
struct WithCost<T>(T, isize);

impl<T: Eq + Ord> PartialOrd for WithCost<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        match (-self.1).cmp(&(-other.1)) {
            Equal => Some(self.0.cmp(&other.0)),
            o => Some(o),
        }
    }
}

/// Abstracted worklist to allow different search strategies/worklist structures
struct Worklist {
    heap: BinaryHeap<WithCost<Graph>>,
}

impl Worklist {
    pub fn from<const N: usize>(xs: [WithCost<Graph>; N]) -> Worklist {
        let heap = BinaryHeap::from(xs);
        Worklist { heap }
    }

    fn push(&mut self, g: WithCost<Graph>) {
        self.heap.push(g);
    }

    fn pop(&mut self) -> Option<WithCost<Graph>> {
        self.heap.pop()
    }

    fn len(&self) -> usize {
        self.heap.len()
    }
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
    let mut counterexamples = HashSet::<Region>::new();

    let worklist = Mutex::new(Worklist::from([WithCost(base, 0)]));
    let regions_tried = ConcurrentHashSet::new();

    // The cache of good covers, to skip re-tiling the same cover
    let good_cover_cache = Mutex::new(LruCache::<CompactRegion, ()>::new(
        NonZero::new(10_000_000).unwrap(),
    ));

    let mut i = 0;
    let n_threads = rayon::current_num_threads();

    loop {
        let graphs = {
            let mut worklist = worklist.lock().unwrap();
            let mut graphs = vec![];
            while graphs.len() < n_threads.max(4) {
                if let Some(graph_and_cost) = worklist.pop() {
                    if regions_tried.contains_sync(&graph_and_cost.0) {
                        continue;
                    }
                    graphs.push(graph_and_cost);
                } else {
                    break;
                }
            }
            graphs
        };
        if graphs.is_empty() {
            break;
        }

        println!("tried {i} graphs (including counterexample refutations)");
        println!("dispatching {} graphs", graphs.len());
        println!("graphs in queue: {}", worklist.lock().unwrap().len());
        i += graphs.len();

        // Each result is a Result that is:
        // - Ok(graph) if the result is fine
        // - Err(Option(counterexample)) if a new counterexample is discovered
        let solutions = graphs
            .into_par_iter()
            .map(|WithCost(graph, curr_cost)| {
                if regions_tried.insert_sync(graph.clone()).is_err() {
                    return Err(None);
                }

                // check if this region is already refuted

                // might want to skip this?
                if !exact_cover_check
                    && counterexamples
                        .iter()
                        .any(|cex| graph.nodes().iter().all(|n| cex.contains(n)))
                {
                    return Err(None);
                }

                let covers = Tiling::min_covers(&graph, &allowed_in_covers, tile_size);

                // for cover in &covers {
                //     println!("{}", serde_json::to_string(&MaybeRegion::from_region(cover.clone())).unwrap());
                // }

                println!("graph: {:?}", graph.nodes());
                println!("#covers: {}", covers.len());
                assert_ne!(covers.len(), 0);

                // check if all covers have a connected metagraph

                // TODO: cache positive results too (can we use suffix trees?)
                let tilings_tried = AtomicUsize::new(0);
                let first_cex = covers.into_par_iter().find_map_any(|cover| {
                    // skip the cover if it is already checked and in cache
                    // this is an expensive operation w.r.t. multithreading
                    {
                        // cannot use `contains` because it does not update the LRU cache.
                        if good_cover_cache.lock().unwrap().get(&cover).is_some() {
                            return None;
                        }
                    }

                    let region = cover.to_region(allowed_in_covers);

                    if exact_cover_check && counterexamples.contains(&region) {
                        return Some(cover);
                    }

                    let g = Graph::from(region);
                    let tilings = Tiling::enumerate(&g, tile_size);
                    let complete = tilings
                        .iter()
                        .filter(|g| g.is_complete())
                        .collect::<Vec<_>>();

                    tilings_tried.fetch_add(complete.len(), Ordering::SeqCst);

                    // check if there is a completion of the partial tiling
                    if complete.iter().any(|tiling| {
                        partial_tile_set.iter().all(|tile| {
                            let color = tiling.color(&tile[0]);
                            tile.iter().all(|n| tiling.color(n) == color)
                        })
                    }) {
                        let first = (*complete.iter().min().unwrap()).clone();
                        let complete_len = complete.len();
                        drop(complete);
                        let reachable = first.reachable(tile_size);

                        if complete_len != reachable.len() {
                            println!("failing cover: {:?}", cover.to_region(allowed_in_covers));
                            Some(cover)
                        } else {
                            // this is a good cover, add it to the cache
                            good_cover_cache.lock().unwrap().put(cover.clone(), ());

                            None
                        }
                    } else {
                        println!("failing cover: {:?}", cover.to_region(allowed_in_covers));
                        // this cover can't be tiled by extending the partial tiling.
                        Some(cover)
                    }
                });

                println!("# tilings tried: {}", tilings_tried.load(Ordering::SeqCst));

                if first_cex.is_none() {
                    return Ok(graph);
                }

                let mut worklist = worklist.lock().unwrap();
                // extend this by one node
                let r = graph.into_region();

                let neighbors = r
                    .neighbors()
                    .expect("the starting graph cannot be empty")
                    .into_iter()
                    .filter(|n| extensions.contains(n))
                    .collect::<Vec<_>>();
                for n in neighbors {
                    let mut new = r.clone();
                    new.insert(n);
                    let new = Graph::from(new);
                    let cost = curr_cost + cost(n);
                    if !regions_tried.contains_sync(&new) {
                        worklist.push(WithCost(new, cost));
                    }
                }

                Err(first_cex)
            })
            .collect::<Vec<_>>();

        for result in solutions {
            match result {
                Ok(graph) => return Some(graph),
                Err(Some(cex)) => {
                    counterexamples.insert(cex.to_region(&allowed_in_covers));
                }
                Err(None) => {}
            }
        }
    }

    None
}

fn main() {
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
                let dx = (n.0 - s.0).abs();
                let dy = (n.1 - s.1).abs();
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
