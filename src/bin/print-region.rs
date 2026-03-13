use clap::Parser;
use triforce::cli::*;
use triforce::*;

/// Visualize the given region
#[derive(Parser, Debug)]
#[command(name = "build-metagraph", version)]
struct Cli {
    /// The region to visualize
    #[arg(required = true)]
    graph: GraphSource,
}

fn main() {
    let cli = Cli::parse();
    let graph = read_graph(cli.graph, false);
    let region = MaybeRegion::from_region(graph.into_region());
    println!("{}", serde_json::to_string(&region).unwrap());
}
