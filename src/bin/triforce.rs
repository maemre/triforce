use clap::Parser;
use triforce::cli::*;
use triforce::*;

#[derive(Parser, Debug)]
#[command(name = "triforce", version)]
struct Cli {
    /// The graph to load
    #[arg(required = true)]
    graph: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,
}

fn main() {
    let cli = Cli::parse();
    let graph = read_graph(cli.graph, true);

    let k = cli.tile_size;

    assert!(
        graph.len().is_multiple_of(k),
        "Size of the graph ({}) is not a multiple of the tile size ({k})",
        graph.len()
    );

    let tilings = Tiling::enumerate(&graph, k);
    println!("partial tilings:      {:>8}", tilings.len());
    let complete = tilings
        .iter()
        .filter(|g| g.is_complete())
        .collect::<Vec<_>>();
    println!("complete tilings:     {:>8}", complete.len());
    let first = (*complete.iter().min().unwrap()).clone();
    drop(complete);
    drop(tilings);
    let reachable = first.reachable(k);
    println!("reachable from first: {:>8}", reachable.len());
}
