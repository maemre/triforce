//! Command-line interface helpers.

use super::*;
use std::{path::PathBuf, str::FromStr};

/// A custom graph source with associated argument parser.
#[derive(Clone, Debug)]
pub enum GraphSource {
    FromFile(PathBuf),
    Triangle(usize),
}

impl FromStr for GraphSource {
    type Err = String;

    // Accept values like: "file=path/to/file", "triangle=5"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, val) = s.split_once('=').ok_or_else(|| {
            "expected format KIND=VALUE (e.g. file=path or triangle=5)".to_string()
        })?;

        match kind.to_ascii_lowercase().as_str() {
            "file" | "from-file" => Ok(GraphSource::FromFile(PathBuf::from(val))),
            "triangle" => {
                let n: usize = val
                    .parse()
                    .map_err(|_| format!("invalid triangle side length `{val}`"))?;
                Ok(GraphSource::Triangle(n))
            }
            other => Err(format!(
                "unknown input kind `{other}` (expected `file` or `triangle`)"
            )),
        }
    }
}

pub fn read_graph(g: GraphSource) -> Graph {
    match g {
        GraphSource::FromFile(file) => {
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
        GraphSource::Triangle(n) => Graph::triangle(n),
    }
}

pub fn read_tiling(file: PathBuf) -> BTreeMap<Node, Color> {
    let tiles = serde_json::from_slice::<Vec<Vec<Node>>>(
        &std::fs::read(file).expect("could not read the input file"),
    )
    .expect("the input file is not a well-formed description of a partial tiling");

    tiles
        .into_iter()
        .enumerate()
        .flat_map(|(i, v)| {
            v.into_iter()
                .map(move |n| (n, Color(NonZeroU8::new(i as u8 + 1).unwrap())))
        })
        .collect()
}
