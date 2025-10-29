use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

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
}

fn search_happy_cover(
    base: Graph,
    extensions: &Graph,
    allowed_in_covers: &Graph,
    partial_tiling: &BTreeMap<Node, Color>,
    tile_size: usize,
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

    let mut worklist = VecDeque::from([base]);
    let mut regions_tried = HashSet::new();

    let mut i = 0;

    while let Some(graph) = worklist.pop_front() {
        i += 1;
        if i % 1000 == 0 {
            println!("Explored {i} alternatives");
        }

        // check if this region is already refuted
        if counterexamples
            .iter()
            .any(|cex| graph.nodes().iter().all(|n| cex.contains(n)))
        {
            continue;
        }

        if !regions_tried.insert(graph.clone()) {
            continue;
        }

        println!("graph: {:?}", graph.nodes());

        let covers = Tiling::min_covers(&graph, &extensions, tile_size);

        println!("#covers: {}", covers.len());

        // check if all covers have a connected metagraph

        // TODO: cache positive results too (can we use suffix trees?)
        let mut tiles = 0;
        let all_connected = covers.into_iter().all(|cover| {
            let g = Graph::from(cover.clone());
            let tilings = Tiling::enumerate(&g, tile_size);
            let complete = tilings
                .iter()
                .filter(|g| g.is_complete())
                .collect::<Vec<_>>();

            tiles += complete.len();

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
                    counterexamples.insert(cover);
                }

                complete_len == reachable.len()
            } else {
                println!("failing cover: {:?}", cover);
                // this cover can't be tiled by extending the partial tiling.
                counterexamples.insert(cover);
                false
            }
        });

        println!("# tilings: {tiles}");

        if all_connected {
            return Some(graph);
        }

        // extend this by one node
        let r = graph.into_region();
        for n in r.neighbors() {
            if extensions.contains(&n) {
                let mut new = r.clone();
                new.insert(n);
                let new = Graph::from(new);
                if !regions_tried.contains(&new) {
                    worklist.push_back(new);
                }
            }
        }
    }

    None
}

fn main() {
    let cli = Cli::parse();
    let base = read_graph(cli.a);
    let mut extensions_r = read_graph(cli.b).into_region();
    extensions_r.append(&mut base.clone().into_region());
    let mut allowed_in_covers_r = read_graph(cli.c).into_region();
    allowed_in_covers_r.append(&mut extensions_r.clone());
    let extensions = Graph::from(extensions_r);
    let allowed_in_covers = Graph::from(allowed_in_covers_r);

    let partial_tiling = cli.partial_tiling.map_or(BTreeMap::new(), read_tiling);

    let k = cli.tile_size;

    match search_happy_cover(base, &extensions, &allowed_in_covers, &partial_tiling, k) {
        None => {
            println!("No suitable region is found");
        }
        Some(r) => {
            println!("found {}", serde_json::to_string(r.nodes()).unwrap());
        }
    }
}
