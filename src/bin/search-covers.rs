use std::collections::{HashSet, VecDeque};

use clap::Parser;
use triforce::*;
use triforce::cli::*;

/// Search for a region C between A and B (A ⊆ C ⊆ B) such that all extensions
/// of C within B have a connected meta-graph.
///
/// This command automatically adds the nodes from A to B before the search.
#[derive(Parser, Debug)]
#[command(name = "gen-covers", version)]
struct Cli {
    /// The graph to load
    #[arg(required = true)]
    a: GraphSource,

    /// Allowed extensions
    #[arg(required = true)]
    b: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,
}

fn search_happy_cover(base: Graph, extensions: &Graph, tile_size: usize) -> Option<Graph> {
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
        if counterexamples.iter().any(|cex| graph.nodes().iter().all(|n| cex.contains(n))) {
            continue;
        }

        if ! regions_tried.insert(graph.clone()) {
            continue;
        }

        let covers = Tiling::min_covers(&graph, &extensions, tile_size);
        
        // check if all covers have a connected metagraph

        // TODO: cache positive results too (can we use suffix trees?)
        let all_connected = covers.into_iter().all(|cover| {
            let g = Graph::from(cover.clone());
            let tilings = Tiling::enumerate(&g, tile_size);
            let complete = tilings.iter().filter(|g| g.is_complete()).collect::<Vec<_>>();
            let first = (*complete.iter().min().unwrap()).clone();
            let complete_len = complete.len();
            drop(complete);
            let reachable = first.reachable(tile_size);

            if complete_len != reachable.len() {
                counterexamples.insert(cover);
            }

            complete_len == reachable.len()
        });

        if all_connected {
            return Some(graph);
        }

        // extend this by one node
        let r = graph.into_region();
        for n in r.neighbors() {
            let mut new = r.clone();
            new.insert(n);
            let new = Graph::from(new);
            if ! regions_tried.contains(&new) {
                worklist.push_back(new);
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
    let extensions = Graph::from(extensions_r);

    let k = cli.tile_size;

    match search_happy_cover(base, &extensions, k) {
        None => { println!("No suitable region is found"); }
        Some(r) => {
            println!("found {}", serde_json::to_string(r.nodes()).unwrap());
        }
    }
}
