use clap::Parser;
use triforce::cli::*;
use triforce::*;

/// Dump the regions R that satisfy:
///
/// 1. A ⊆ R ⊆ B
/// 2. R.len() is divisible by tile_size
#[derive(Parser, Debug)]
#[command(name = "check-covers", version)]
struct Cli {
    /// The fixed region (nodes required in and allowed in the fixed region)
    #[arg(required = true)]
    a: GraphSource,

    /// Nodes allowed in covers of the fixed region
    #[arg(required = true)]
    b: GraphSource,

    /// Size of the tiles to tile the graph with
    #[arg(required = true)]
    tile_size: usize,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let base = read_graph(cli.a, false);
    let mut allowed_in_covers_r = read_graph(cli.b, true).into_region();
    allowed_in_covers_r.append(&mut base.clone().into_region());
    let allowed_in_covers = Graph::from(allowed_in_covers_r);

    assert!(
        base.len() <= BYTES_IN_COMPACT_REGION * 8,
        "{} > {}",
        base.len(),
        BYTES_IN_COMPACT_REGION * 8
    );

    let potential_covers = Tiling::potential_covers(&base, &allowed_in_covers, cli.tile_size)
        .iter()
        .map(|cr| MaybeRegion::from_region(cr.to_region(&allowed_in_covers)))
        .collect::<Vec<_>>();

    println!("{}", serde_json::to_string(&potential_covers).unwrap());
}
