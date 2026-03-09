use ahash::{HashMap, HashSet};
use itertools::Itertools;
use log::info;
use petgraph::dot::{self, Dot};
use rustworkx_core::connectivity::connected_components;
use std::path::PathBuf;
use triforce::graph::diameter;
use triforce::metagraph::Metagraph;
use triforce::viz::mk_hex;

use clap::Parser;
use std::str::FromStr;
use triforce::cli::*;
use triforce::*;

/// Extract a path along the diameter of the metagraph for the given graph
#[derive(Parser, Debug)]
#[command(name = "build-metagraph", version)]
struct Cli {
    /// The graph whose metagraph we will build
    #[arg(required = true)]
    graph: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,

    /// Where to save the images in the graph
    output: PathBuf,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let base_graph = read_graph(cli.graph, false);

    println!(
        "Loaded:\n{}",
        serde_json::ser::to_string(&MaybeRegion::from_region(base_graph.clone().into_region()))
            .unwrap()
    );

    let tile_size = cli.tile_size;

    assert!(tile_size > 1, "Tile size must be greater than 1");
    assert!(
        base_graph.len() <= BYTES_IN_COMPACT_REGION * 8,
        "{} > {}",
        base_graph.len(),
        BYTES_IN_COMPACT_REGION * 8
    );

    info!("building the metagraph...");
    let metagraph = Metagraph::new(&base_graph, tile_size);

    println!("There are {} nodes", metagraph.nodes.len());
    println!("There are {} edges", metagraph.meta.edge_count());

    let cc = connected_components(&metagraph.meta);
    assert!(cc.len() > 0, "There are {} connected components", cc.len());

    let ls_path = diameter(&metagraph.meta);
    println!("The diameter is {}", ls_path.len());

    // generate the tilings
    let tilings = ls_path
        .iter()
        .map(|node_idx| {
            let i = metagraph.meta.node_weight(*node_idx).unwrap();
            let tiling = &metagraph.nodes[*i];
            let tiles = tiling
                .graph
                .nodes()
                .iter()
                .map(|node| {
                    (
                        mk_hex(node.0 as i32, node.1 as i32),
                        tiling.color_for_rendering(node).unwrap(),
                    )
                })
                .collect::<HashMap<_, _>>();
            (i.to_string(), tiles)
        })
        .collect::<Vec<_>>();

    // let unique = tilings.iter().map(|p| p.1.iter().map(|(h, c)| (*h, *c)).collect::<Vec<_>>()).collect::<HashSet<_>>();
    // println!("unique tilings: {}", unique.len());

    viz::render(
        viz::RenderData { tilings },
        format!("{}", cli.output.as_path().to_str().unwrap()),
    )
}
