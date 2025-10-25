use clap::{Parser, Subcommand};
use triforce::*;

/// Example CLI that accepts either a file or a number as input.
#[derive(Parser, Debug)]
#[command(name = "triforce", version)]
struct Cli {
    /// The graph to load
    #[command(subcommand)]
    graph: InputGraph,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,
}

#[derive(Subcommand, Debug)]
enum InputGraph {
    /// Read input from a file
    FromFile {
        /// Path to the file
        #[arg(value_name = "FILE")]
        file: String,
    },

    /// Create a triangle with given side length
    Triangle {
        /// Side length
        #[arg(value_name = "N")]
        n: usize,
    },
}

fn main() {
    let cli = Cli::parse();
    let graph = match cli.graph {
        InputGraph::FromFile { file } => {
            let region =
		serde_json::from_slice::<MaybeRegion>(
		    &std::fs::read(file)
			.expect("could not read the input file")
		)
		.expect("the input file is not a well-formed description of a region")
		.to_region()
		.expect("the input region is not well-structured (does not start at origin or has duplicates)");
            Graph::from(region)
        }
        InputGraph::Triangle { n } => Graph::triangle(n),
    };

    let k = cli.tile_size;

    assert!(
        graph.len() % k == 0,
        "Size of the graph ({}) is not a multiple of the tile size ({k})",
        graph.len()
    );

    let tilings = Tiling::enumerate(&graph, k);
    println!("partial tilings:      {:>8}", tilings.len());
    let complete = tilings.iter().filter(|g| g.is_complete()).collect::<Vec<_>>();
    println!("complete tilings:     {:>8}", complete.len());
    let first = (*complete.iter().min().unwrap()).clone();
    drop(complete);
    drop(tilings);
    let reachable = first.reachable(k);
    println!("reachable from first: {:>8}", reachable.len());
}
