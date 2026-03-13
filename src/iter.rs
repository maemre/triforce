//! Custom iterators to calculate tilings, etc. on the fly.
#![allow(dead_code)]

use std::{sync::Arc, thread::Scope};

use scc::HashSet as ConcurrentHashSet;

use crate::{
    concurrency::{WithCost, Worklist},
    *,
};

/// An iterator that streams tilings.
pub struct TilingIterator<'g> {
    /// The base graph to tile
    pub graph: &'g Graph,
    /// The nodes allowed in covers
    pub allowed_in_covers: &'g Graph,
    /// Tile size for this tiling
    pub tile_size: usize,
    /// Tilings we have already seen
    pub seen: Arc<ConcurrentHashSet<CompactRegion>>,
    /// Tilings to send as an iterator
    pub fresh: Vec<CompactRegion>,
    /// Tilings we need to process, this is an option because of cyclic lifetime issues.
    pub worklist: Worklist<CompactRegion>,
    /// All tiles of the given tile size
    tiles: Set<Region>,
}

impl<'g> TilingIterator<'g> {
    /// Create a fresh iterator
    pub fn new(graph: &'g Graph, allowed_in_covers: &'g Graph, tile_size: usize) -> Self {
        let n_threads = rayon::current_num_threads();
        let seen = Arc::new(ConcurrentHashSet::<CompactRegion>::new());
        let seen2 = seen.clone();

        TilingIterator {
            graph,
            allowed_in_covers,
            tile_size,
            tiles: regions(tile_size),
            seen,
            fresh: Vec::new(),
            worklist: Worklist::new([WithCost(CompactRegion::empty(), 0)], n_threads, seen2),
        }
    }

    /// Get the covers, this should be run outside the scope given to
    /// `Self::start`.  Otherwise, it would race with the threads doing work.
    pub fn get_covers(&self) -> HashSet<CompactRegion> {
        let g_compact =
            CompactRegion::from(&self.graph.clone().into_region(), self.allowed_in_covers);
        let mut result = HashSet::new();
        self.seen.iter_sync(|cr: &CompactRegion| {
            if cr.is_superset_of(g_compact) {
                result.insert(*cr);
            }
            true
        });
        result
    }

    /// Adapted version of `par_min_covers` that makes progress using threads spawned within the given scope.
    pub fn start(&'g self, scope: &'g Scope<'g, '_>) {
        use crate::concurrency::{Task, WithCost};
        let g = self.graph;
        let tile_size = self.tile_size;
        let allowed_in_covers = self.allowed_in_covers;
        let n_threads = rayon::current_num_threads();

        assert!(!g.nodes.is_empty());
        assert!(tile_size > 0);
        assert!(
            g.nodes.iter().all(|n| allowed_in_covers.contains(n)),
            "the extension must be a superset of the graph"
        );

        // Opt 2: Precompute per-node neighbor bitmasks in allowed_in_covers.
        let n_nodes = allowed_in_covers.len();
        let mut neighbor_masks = vec![0u128; n_nodes];
        for (i, node) in allowed_in_covers.nodes.iter().enumerate() {
            for nbr in neighbors(node) {
                if let Some(&j) = allowed_in_covers.indices.get(&nbr) {
                    neighbor_masks[i] |= 1u128 << j;
                }
            }
        }
        // Bitmask of g's nodes within allowed_in_covers.
        let g_mask: u128 = g
            .nodes
            .iter()
            .map(|n| 1u128 << allowed_in_covers.indices[n])
            .fold(0, |a, b| a | b);
        // Seed bit: first node of g in allowed_in_covers.
        let g_seed_bit: u128 =
            1u128 << allowed_in_covers.indices[g.indices.first_key_value().unwrap().0];

        // Opt 3: For each node in g (by g index), precompute compact tile placement masks
        // that cover that node and fit entirely within allowed_in_covers.
        let tiles_for_g_node: Vec<Vec<u128>> = g
            .nodes
            .iter()
            .map(|n| {
                let mut masks = vec![];
                for t in &self.tiles {
                    for n_t in &t.inner {
                        let shifted = shift(sub(*n, *n_t), t);
                        if shifted.iter().all(|m| allowed_in_covers.contains(m)) {
                            let mask = shifted
                                .iter()
                                .map(|m| 1u128 << allowed_in_covers.indices[m])
                                .fold(0u128, |a, b| a | b);
                            masks.push(mask);
                        }
                    }
                }
                masks.sort_unstable();
                masks.dedup();
                masks
            })
            .collect();

        // Map from allowed_in_covers bit-index to g.nodes index (usize::MAX if not in g).
        let mut allowed_idx_to_g_idx = vec![usize::MAX; n_nodes];
        for (gi, n) in g.nodes.iter().enumerate() {
            allowed_idx_to_g_idx[allowed_in_covers.indices[n]] = gi;
        }

        for _ in 0..n_threads {
            // clone the per-thread data so it can outlive the current function.
            let tiles_for_g_node = tiles_for_g_node.clone();
            let allowed_idx_to_g_idx = allowed_idx_to_g_idx.clone();
            let neighbor_masks = neighbor_masks.clone();

            scope.spawn(move || {
                loop {
                    let WithCost(compact_region, _) = match self.worklist.pop() {
                        Task::Done => return,
                        Task::Todo(x) => x,
                    };

                    // Opt 2: Compute frontier as a bitmask — neighbors of the current
                    // region that are in g but not yet in the region.
                    let frontier_mask: u128 = if compact_region.is_empty() {
                        g_seed_bit
                    } else {
                        let mut m = 0u128;
                        let mut bits = compact_region.0;
                        while bits != 0 {
                            let i = bits.trailing_zeros() as usize;
                            m |= neighbor_masks[i];
                            bits &= bits - 1;
                        }
                        m & g_mask & !compact_region.0
                    };

                    // Opt 3: Iterate frontier bits and look up precomputed tile masks.
                    let mut children = vec![];
                    let mut frontier = frontier_mask;
                    while frontier != 0 {
                        let ai = frontier.trailing_zeros() as usize;
                        frontier &= frontier - 1;
                        let gi = allowed_idx_to_g_idx[ai];
                        for &tile_mask in &tiles_for_g_node[gi] {
                            if compact_region.0 & tile_mask == 0 {
                                let combined = CompactRegion(compact_region.0 | tile_mask);
                                if !self.seen.contains_sync(&combined) {
                                    children.push(WithCost(combined, 0));
                                }
                            }
                        }
                    }
                    self.worklist.push_all(children);
                }
            });
        }
    }
}
