use ahash::HashMap;
use itertools::Itertools;
use log::info;
use petgraph::dot::{self, Dot};
use rustworkx_core::connectivity::connected_components;
use std::path::PathBuf;
use triforce::metagraph::Metagraph;
use triforce::viz::mk_hex;

use clap::Parser;
use std::str::FromStr;
use triforce::cli::*;
use triforce::*;

/// Build the metagraph for the given graph
#[derive(Parser, Debug)]
#[command(name = "build-metagraph", version)]
struct Cli {
    /// The graph whose metagraph we will build
    #[arg(required = true)]
    graph: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,

    /// Where to save the metagraph as a JSON file.
    output: PathBuf,

    /// Dump the smallest connected component
    #[arg(long)]
    dump_cc: bool,
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

    info!("writing the metagraph...");
    std::fs::write(
        cli.output.clone(),
        &serde_json::ser::to_vec(&metagraph.meta).unwrap(),
    )
    .unwrap();

    println!("There are {} nodes", metagraph.nodes.len());
    println!("There are {} edges", metagraph.meta.edge_count());

    let cc = connected_components(&metagraph.meta);
    println!("There are {} connected components", cc.len());

    let mut by_size = cc
        .iter()
        .counts_by(|s| s.len())
        .into_iter()
        .collect::<Vec<_>>();
    by_size.sort();

    println!("    size, #components");
    for (len, count) in &by_size {
        println!("{:>8}, {:>11}", len, count);
    }

    println!("components in detail:");
    println!("#nodes, #edges");
    for s in &cc {
        let n_nodes = s.len();
        let n_edges: usize = s.iter().map(|idx| metagraph.meta.edges(*idx).count()).sum();
        println!("{n_nodes:>5}, {n_edges:>5}");
    }

    if cli.dump_cc {
        println!("the smallest connected component:");
        let smallest_cc = cc.iter().find(|s| s.len() == by_size[0].0).unwrap();
        let mut smallest_cc_graph = metagraph.meta.clone();
        smallest_cc_graph.retain_nodes(|_, idx| smallest_cc.contains(&idx));

        let dot = Dot::with_attr_getters(
            &smallest_cc_graph,
            &[dot::Config::EdgeNoLabel, dot::Config::NodeNoLabel],
            &|_, _| String::new(),
            &|_, (_, s)| format!(r#"label = "{s}""#),
        );
        println!("{:?}", dot);

        // generate the tilings
        let mut tilings = smallest_cc_graph
            .node_weights()
            .map(|i| {
                let tiling = &metagraph.nodes[*i];
                let tiles = tiling
                    .graph
                    .nodes()
                    .iter()
                    .map(|node| {
                        (
                            mk_hex(node.0 as i32, node.1 as i32),
                            tiling.color(node).unwrap(),
                        )
                    })
                    .collect::<HashMap<_, _>>();
                (i.to_string(), tiles)
            })
            .collect::<Vec<_>>();
        tilings.sort_by_key(|(i, _)| -i32::from_str(i).unwrap());

        viz::render(
            viz::RenderData { tilings },
            format!("{}-scc", cli.output.as_path().to_str().unwrap()),
        )
    }
}
