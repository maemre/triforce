use clap::Parser;
use std::path::PathBuf;
use triforce::cli::*;
use triforce::{viz::*, *};

/// Visualize the given region
#[derive(Parser, Debug)]
#[command(name = "build-metagraph", version)]
struct Cli {
    /// The region to visualize
    #[arg(required = true)]
    graph: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let name = cli.graph.as_path().to_str().unwrap().to_string();
    let graph = read_graph(GraphSource::FromFile(cli.graph), false);

    let tilings = vec![(
        name.clone(),
        graph
            .nodes()
            .iter()
            .map(|(x, y)| (mk_hex(*x as i32, *y as i32), Color::new(1)))
            .collect(),
    )];

    render(RenderData { tilings }, format!("{name}.out"));
}
