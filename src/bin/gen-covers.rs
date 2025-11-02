use clap::Parser;
use triforce::cli::*;
use triforce::*;

/// Generate minimal covers of the given graph (possible tilings that use any
/// nodes from extensions to cover graph)
///
/// This command automatically adds the nodes from the graph to the extension.
#[derive(Parser, Debug)]
#[command(name = "gen-covers", version)]
struct Cli {
    /// The graph to load
    #[arg(required = true)]
    graph: GraphSource,

    /// Allowed extensions
    #[arg(required = true)]
    extensions: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,
}

fn main() {
    let cli = Cli::parse();
    let graph = read_graph(cli.graph, false);
    let mut extensions_r = read_graph(cli.extensions, true).into_region();
    extensions_r.append(&mut graph.clone().into_region());
    let extensions = Graph::from(extensions_r);

    let k = cli.tile_size;

    let covers = Tiling::min_covers(&graph, &extensions, k);
    println!("# covers: {:>8}", covers.len());

    for c in covers.iter().take(10) {
        println!("{:?}", c.to_region(&extensions));
    }
}
