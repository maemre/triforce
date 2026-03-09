//! Assorted graph processing utilities.  These work with arbitrary `petgraph`
//! graphs rather than specifically on metagraphs.

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::visit::IntoNodeIdentifiers;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use rustworkx_core::connectivity::{connected_components, stoer_wagner_min_cut};
use rustworkx_core::petgraph::graph::{NodeIndex, UnGraph};
use rustworkx_core::petgraph::visit::EdgeRef;
use rustworkx_core::shortest_path::single_source_all_shortest_paths;

// Return the longest shortest path starting at given node.
//
// This method uses breadth-first search so g must be unweighted.
fn longest_shortest_path<N>(
    g: &UnGraph<N, ()>,
    start: NodeIndex
) -> Vec<NodeIndex> {
    let mut predecessor: Vec<Option<NodeIndex>> = vec![None; g.node_count()];
    let mut visited = HashSet::<NodeIndex>::new();
    let mut worklist = VecDeque::from([start]);
    let mut distance: Vec<Option<u32>> = vec![None; g.node_count()];
    distance[start.index()] = Some(0);

    while let Some(node) = worklist.pop_front() {
        if ! visited.insert(node) {
            continue;
        }

        let d = distance[node.index()].unwrap();
        for next in g.neighbors(node) {
            if let Some(d_old) = distance[next.index()] && d_old < d + 1 {
                continue;
            }
            distance[next.index()] = Some(d + 1);
            predecessor[next.index()] = Some(node);
            worklist.push_back(next);
        }
    }

    let longest = NodeIndex::new(distance.iter().enumerate().max_by_key(|p| p.1).unwrap().0);

    let mut path = vec![longest];
    let mut current = longest;
    while current != start {
        current = predecessor[current.index()].unwrap();
        path.push(current);
    }

    path.reverse();
    path
}

/// Calculate a path along the diameter naively
pub fn diameter<N: Send + Sync>(
    g: &UnGraph<N, ()>,
) -> Vec<NodeIndex> {
    g.node_indices().par_bridge().map(|start| {
        longest_shortest_path(g, start)
    }).max_by_key(|v| v.len()).unwrap()
}

// Bottleneck calculation via edge connectivity

/// A cut in an unweighted graph
#[derive(Debug, Clone)]
pub struct Cut {
    /// one of the sides of the cut, as a reference to the nodes in the whole graph
    pub side_a: Vec<NodeIndex>,
    /// edges in the cut
    pub cut_edges: Vec<(NodeIndex, NodeIndex)>,
}

/// Build an induced subgraph on `nodes`, with maps between original <-> subgraph indices.
fn induced_subgraph(
    g: &UnGraph<(), ()>,
    nodes: &[NodeIndex],
) -> (UnGraph<(), ()>, Vec<NodeIndex>, HashMap<NodeIndex, NodeIndex>) {
    let mut sub = UnGraph::<(), ()>::new_undirected();

    // Map original nodes -> subgraph nodes
    let mut orig_to_sub: HashMap<NodeIndex, NodeIndex> = HashMap::with_capacity(nodes.len());
    // Map subgraph nodes -> original nodes (index by subgraph NodeIndex::index())
    let mut sub_to_orig: Vec<NodeIndex> = Vec::with_capacity(nodes.len());

    for &u in nodes {
        let su = sub.add_node(());
        orig_to_sub.insert(u, su);
        sub_to_orig.push(u);
    }

    // Add edges that stay inside the induced node set
    let node_set: HashSet<NodeIndex> = nodes.iter().copied().collect();
    for e in g.edge_references() {
        let (u, v) = (e.source(), e.target());
        if node_set.contains(&u) && node_set.contains(&v) {
            let su = orig_to_sub[&u];
            let sv = orig_to_sub[&v];
            sub.add_edge(su, sv, ());
        }
    }

    (sub, sub_to_orig, orig_to_sub)
}

fn cut_edges_in_original(
    g: &UnGraph<(), ()>,
    active_nodes: &HashSet<NodeIndex>,
    side_a: &HashSet<NodeIndex>,
) -> Vec<(NodeIndex, NodeIndex)> {
    let mut cut = Vec::new();
    for e in g.edge_references() {
        let (u, v) = (e.source(), e.target());
        if !active_nodes.contains(&u) || !active_nodes.contains(&v) {
            continue;
        }
        let ua = side_a.contains(&u);
        let va = side_a.contains(&v);
        if ua ^ va {
            cut.push((u, v));
        }
    }
    cut
}

/// Recursively decompose by increasing bottleneck strength (smallest cuts first).
///
/// - `k_stop`: stop splitting once min-cut >= k_stop.
/// - `min_size`: stop splitting if subgraph has fewer than `min_size` nodes.
pub fn bottleneck_decompose(
    g: &UnGraph<(), ()>,
    k_stop: usize,
    min_size: usize,
) -> Vec<Cut> {
    let all_nodes: Vec<NodeIndex> = g.node_indices().collect();
    let mut out = Vec::new();
    decompose_rec(g, &all_nodes, k_stop, min_size, &mut out);
    out
}

fn decompose_rec(
    g: &UnGraph<(), ()>,
    nodes: &[NodeIndex],
    k_stop: usize,
    min_size: usize,
    out: &mut Vec<Cut>,
) {
    if nodes.len() < 2 || nodes.len() < min_size {
        return;
    }

    // Work on induced subgraph
    let (sub, sub_to_orig, _orig_to_sub) = induced_subgraph(g, nodes);

    // Global min cut (unweighted -> each edge cost 1)
    let res = stoer_wagner_min_cut(&sub, |_| Ok::<usize, ()>(1)).unwrap();

    let Some((cut_value, part_sub)) = res else {
        return; // fewer than 2 nodes in subgraph (guarded above anyway)
    };

    // Handle disconnected subgraph
    if cut_value == 0 {
        // Split by connected components and recurse.
        // connected_components returns Vec<HashSet<NodeId>> for the *subgraph*
        let comps = connected_components(&sub);
        if comps.len() <= 1 {
            return;
        }
        for comp_sub in comps {
            let mut comp_orig = Vec::with_capacity(comp_sub.len());
            for su in comp_sub {
                let ou = sub_to_orig[su.index()];
                comp_orig.push(ou);
            }
            decompose_rec(g, &comp_orig, k_stop, min_size, out);
        }
        return;
    }

    // Stop criterion: "already well-connected enough"
    if cut_value >= k_stop {
        return;
    }

    // Convert partition back to original nodes
    let side_a_vec: Vec<NodeIndex> = part_sub
        .iter()
        .map(|&su| sub_to_orig[su.index()])
        .collect();

    let side_a_set: HashSet<NodeIndex> = side_a_vec.iter().copied().collect();
    let active_set: HashSet<NodeIndex> = nodes.iter().copied().collect();

    // Extract actual cut edges in the ORIGINAL graph
    let cut_edges = cut_edges_in_original(g, &active_set, &side_a_set);

    assert_eq!(cut_value, cut_edges.len());

    out.push(Cut {
        side_a: side_a_vec.clone(),
        cut_edges: cut_edges.clone(),
    });

    // Recurse into both sides
    let mut side_b = Vec::new();
    side_b.reserve(nodes.len().saturating_sub(side_a_set.len()));
    for &u in nodes {
        if !side_a_set.contains(&u) {
            side_b.push(u);
        }
    }

    decompose_rec(g, &side_a_vec, k_stop, min_size, out);
    decompose_rec(g, &side_b, k_stop, min_size, out);
}
