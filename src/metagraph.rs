//! Metagraph processing utilities

use crate::*;
use log::{info, warn};
use petgraph::graph::UnGraph;

/// A graph of tilings connected via recomb moves, i.e. the transition system
/// that underlies the recomb Markov chain.
///
/// Nodes are referenced via their indices in the graph.
#[derive(Debug, Clone)]
pub struct Metagraph<'graph> {
    pub nodes: Vec<Tiling<'graph>>,
    // the actual graph structure
    pub meta: UnGraph<usize, ()>,
}

impl<'g> Metagraph<'g> {
    pub fn new(g: &'g Graph, tile_size: usize) -> Metagraph<'g> {
        info!("pre-calculating recomb sets...");
        let recomb = recomb(tile_size);
        info!("enumerating partial tilings...");
        let partial_tilings = Tiling::enumerate(g, tile_size);

        info!("adding nodes...");
        let complete_tilings = partial_tilings
            .into_iter()
            .filter(|g| g.is_complete())
            .collect::<Vec<_>>();

        let inverted_idx = complete_tilings
            .iter()
            .enumerate()
            .map(|(i, t)| (t, i))
            .collect::<HashMap<_, _>>();

        let mut meta = UnGraph::with_capacity(complete_tilings.len(), complete_tilings.len());

        let nodes = complete_tilings
            .iter()
            .enumerate()
            .map(|(i, _)| meta.add_node(i))
            .collect::<Vec<_>>();

        info!("adding edges...");
        let mut n_edges = 0;
        for (i, tiling) in complete_tilings.iter().enumerate() {
            for neighbor in tiling.neighbors(&recomb) {
                let j = inverted_idx[&neighbor];
                if i >= j {
                    // don't add loops or back edges
                    continue;
                }
                meta.add_edge(nodes[i], nodes[j], ());
                n_edges += 1;
            }
        }
        info!("added {n_edges} edges");

        drop(inverted_idx);

        Metagraph {
            nodes: complete_tilings,
            meta,
        }
    }
}
