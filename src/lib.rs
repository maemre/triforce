//! The main library.  Exports the interface used by different commands.
//!
//! We use the following encoding for a triangular lattice:
//!
//! - Nodes are represented by (x, y) coordinates.
//! - y is even if x is even.
//! - y is odd if x is odd.
//! - x is ordered left-to-right
//! - y is ordered top-to-bottom
//!
//! So, the coordinates look like
//!
//! ```ignore
//!
//! (0,0)
//!      (1,1)
//! (0,2)     (2,2)
//!      (1,3)
//! (0,4)
//!
//! ```
//!
//! Coordinates are ordered in lexicographic order, and we normalize regions to
//! start at the origin in many data structures below.

use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use rayon::iter::*;
use std::{
    collections::{BTreeMap, BTreeSet as Set},
    num::NonZeroU8,
    ops::{BitOr, Index},
    sync::atomic::AtomicUsize,
};

use serde::{Deserialize, Serialize};

pub mod cli;
pub mod concurrency;
mod fmt;
pub mod graph;
// pub mod iter;
mod macros;
pub mod metagraph;
pub mod viz;

#[allow(unused_imports)]
pub use fmt::*;
#[allow(unused_imports)]
use macros::*;

// A node is just a pair of coordinates
pub type Node = (i8, i8);

// A region is just a (connected) set of nodes
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
pub struct Region {
    inner: Vec<Node>,
}

impl Default for Region {
    fn default() -> Self {
        Self::new()
    }
}

impl Region {
    pub fn new() -> Self {
        Self::from(vec![])
    }

    pub fn from(mut inner: Vec<Node>) -> Self {
        inner.sort();
        Region { inner }
    }

    pub fn contains(&self, n: &Node) -> bool {
        self.inner.contains(n)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Node> {
        self.inner.iter()
    }

    pub fn insert(&mut self, new: Node) -> bool {
        if self.contains(&new) {
            false
        } else {
            self.inner.push(new);
            self.inner.sort();
            true
        }
    }

    pub fn remove(&mut self, node_to_remove: &Node) -> bool {
        if let Some(i) = self.inner.iter().position(|n| n == node_to_remove) {
            self.inner.remove(i);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn first(&self) -> Option<&Node> {
        self.inner.first()
    }

    pub fn append(&mut self, r2: &mut Region) {
        self.inner.append(&mut r2.inner);
        self.inner.sort();
        self.inner.dedup();
    }

    /// Neighbors of this region.
    ///
    /// For an empty region, this method returns None.
    pub fn neighbors(&self) -> Option<Set<Node>> {
        if self.is_empty() {
            None
        } else {
            Some(self.inner.iter().flat_map(neighbors).collect())
        }
    }
}

impl FromIterator<Node> for Region {
    fn from_iter<T: IntoIterator<Item = Node>>(iter: T) -> Self {
        Region::from(Vec::from_iter(iter))
    }
}

impl IntoIterator for Region {
    type Item = Node;

    type IntoIter = std::vec::IntoIter<Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a> IntoIterator for &'a Region {
    type Item = &'a Node;

    type IntoIter = std::slice::Iter<'a, Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl BitOr for &Region {
    type Output = Region;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut r = self.clone();
        r.inner.extend(rhs.inner.iter().cloned());
        r.inner.sort();
        r.inner.dedup();
        r
    }
}

pub const BYTES_IN_COMPACT_REGION: usize = size_of::<u128>();

/// Compact representation of a region as a bitset.
/// Needs the allowed nodes to be converted into a region.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompactRegion(u128);

impl CompactRegion {
    pub fn empty() -> CompactRegion {
        CompactRegion(0)
    }

    pub fn from(r: &Region, universe: &Graph) -> CompactRegion {
        debug_assert!(universe.len() < BYTES_IN_COMPACT_REGION * 8);
        let mut bitset = 0;
        for (i, n) in universe.nodes.iter().enumerate() {
            bitset |= (r.contains(n) as u128) << i;
        }
        debug_assert_eq!(&CompactRegion(bitset).to_region(universe), r);
        CompactRegion(bitset)
    }

    pub fn to_region(&self, universe: &Graph) -> Region {
        debug_assert!(universe.len() < BYTES_IN_COMPACT_REGION * 8);
        Region::from(
            universe
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| {
                    if self.0 & (1 << i) != 0 {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    #[inline(always)]
    pub fn contains(&self, n: &Node, universe: &Graph) -> bool {
        debug_assert!(universe.len() < BYTES_IN_COMPACT_REGION * 8);
        let result = self.0 & (1 << universe.indices[n]) != 0;
        debug_assert_eq!(self.to_region(universe).contains(n), result);
        result
    }

    /// Number of nodes in this region.
    #[inline(always)]
    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether this region is empty.
    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Union of two compact regions (both must share the same universe).
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn bitor(self, other: Self) -> Self {
        CompactRegion(self.0 | other.0)
    }

    /// True if every bit in `other` is also set in `self`.
    #[inline(always)]
    pub fn is_superset_of(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// True if `self` and `other` share no bits (disjoint regions).
    #[inline(always)]
    pub fn is_disjoint(self, other: Self) -> bool {
        self.0 & other.0 == 0
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn compact_region_from_to_region() {
        let universe = Graph::triangle(5);
        let r = Region::from(vec![(0, 0), (0, 2), (1, 1), (2, 2), (3, 5), (4, 4)]);
        assert_eq!(r, CompactRegion::from(&r, &universe).to_region(&universe));
    }

    /// Check that par_min_covers returns the same set as min_covers for the given graph/extension.
    fn assert_par_min_covers_eq(g: &Graph, allowed: &Graph, tile_size: usize) {
        let seq = Tiling::min_covers(g, allowed, tile_size, &HashSet::default());
        let par = Tiling::par_min_covers(g, allowed, tile_size);
        assert_eq!(
            seq.is_none(),
            par.is_none(),
            "both should agree on None/Some"
        );
        if let (Some(seq_set), Some(par_set)) = (seq, par) {
            assert_eq!(seq_set, par_set, "cover sets should be identical");
        }
    }

    #[test]
    fn par_min_covers_equiv_triangle3_4() {
        let g = Graph::triangle(3);
        let allowed = Graph::triangle(4);
        assert_par_min_covers_eq(&g, &allowed, 2);
    }

    #[test]
    fn par_min_covers_equiv_triangle4_5() {
        let g = Graph::triangle(4);
        let allowed = Graph::triangle(5);
        assert_par_min_covers_eq(&g, &allowed, 2);
    }

    #[test]
    fn par_min_covers_equiv_triangle3_4_tile3() {
        let g = Graph::triangle(3);
        let allowed = Graph::triangle(4);
        assert_par_min_covers_eq(&g, &allowed, 3);
    }

    #[test]
    fn compact_region_ord() {
        // empty < non-empty
        let universe = Graph::triangle(4);
        let empty = CompactRegion::empty();
        let r1 = Region::from(vec![(0, 0)]);
        let r2 = Region::from(vec![(0, 0), (1, 1)]);
        let cr1 = CompactRegion::from(&r1, &universe);
        let cr2 = CompactRegion::from(&r2, &universe);
        assert!(empty < cr1);
        assert!(cr1 < cr2);
        // reflexive
        assert_eq!(empty.cmp(&empty), std::cmp::Ordering::Equal);
        assert_eq!(cr1.cmp(&cr1), std::cmp::Ordering::Equal);
    }

    #[test]
    fn compact_region_bitops() {
        let universe = Graph::triangle(4);
        let r1 = Region::from(vec![(0, 0), (1, 1)]);
        let r2 = Region::from(vec![(0, 2), (1, 1)]);
        let r3 = Region::from(vec![(0, 0), (0, 2), (1, 1)]);
        let cr1 = CompactRegion::from(&r1, &universe);
        let cr2 = CompactRegion::from(&r2, &universe);
        let cr3 = CompactRegion::from(&r3, &universe);
        let empty = CompactRegion::empty();

        // len
        assert_eq!(empty.len(), 0);
        assert_eq!(cr1.len(), 2);
        assert_eq!(cr3.len(), 3);

        // is_empty
        assert!(empty.is_empty());
        assert!(!cr1.is_empty());

        // bitor (union)
        assert_eq!(cr1.bitor(cr2).to_region(&universe), r3);

        // is_superset_of
        assert!(cr3.is_superset_of(cr1));
        assert!(cr3.is_superset_of(cr2));
        assert!(!cr1.is_superset_of(cr3));
        assert!(cr1.is_superset_of(empty));
        assert!(cr1.is_superset_of(cr1));

        // is_disjoint
        let r_disjoint = Region::from(vec![(0, 2)]);
        let cr_disjoint = CompactRegion::from(&r_disjoint, &universe);
        assert!(cr1.is_disjoint(cr_disjoint)); // (0,0),(1,1) vs (0,2)
        assert!(!cr1.is_disjoint(cr2)); // share (1,1)
        assert!(cr1.is_disjoint(empty));
    }
}

/// Representation of a region for serialization/deserialization.
#[derive(Serialize, Deserialize)]
pub struct MaybeRegion(pub Vec<Node>);

impl MaybeRegion {
    pub fn to_region(mut self, required_to_start_at_origin: bool) -> Option<Region> {
        self.0.sort();
        let l = self.0.len();
        self.0.dedup();

        if self.0.len() != l {
            return None;
        }

        if let Some(n) = self.0.first()
            && required_to_start_at_origin
            && *n != (0, 0)
        {
            return None;
        }

        Some(Region::from(self.0))
    }

    pub fn from_region(r: Region) -> MaybeRegion {
        MaybeRegion(r.inner)
    }
}

// This returns neighbors in an infinite lattice
pub fn neighbors(n: &Node) -> [Node; 6] {
    let (x, y) = *n;
    [
        (x - 1, y - 1),
        (x - 1, y + 1),
        (x, y - 2),
        (x, y + 2),
        (x + 1, y - 1),
        (x + 1, y + 1),
    ]
}

// Generate all connected regions of size n where the lexicographically smallest
// cells is at the origin.
pub fn regions(n: usize) -> Set<Region> {
    debug_assert!(n > 0);
    if n == 1 {
        return Set::from([Region::from(vec![(0, 0)])]);
    }

    // get regions of size N and add a neighbor
    regions(n - 1)
        .into_iter()
        .flat_map(|init| {
            init.iter()
                .flat_map(neighbors)
                .filter_map(|new| {
                    if new.0 < 0 || (new.0 == 0 && new.1 <= 0) || init.contains(&new) {
                        None
                    } else {
                        let mut r = init.clone();
                        r.insert(new);
                        Some(r)
                    }
                })
                .collect::<Vec<Region>>()
        })
        .collect()
}

// Shift each node in `r` by `n` (move the origin to the coordinates of `n`)
fn shift(n: Node, r: &Region) -> Region {
    r.iter().map(|(x, y)| (n.0 + *x, n.1 + *y)).collect()
}

// Calculate the vector n1 - n2 as a node
fn sub(n1: Node, n2: Node) -> Node {
    (n1.0 - n2.0, n1.1 - n2.1)
}

// Calculate recombinations of all pairs of regions of size `n`
//
// The return value is a map where:
//
// - keys are the larger region of size n * 2
//
// - values are potential ways of splitting that region into two regions of size n
//
pub fn recomb(n: usize) -> BTreeMap<Region, Set<(Region, Region)>> {
    // Generate all regions of size n
    let rs = regions(n);

    let mut result = BTreeMap::<Region, Set<(Region, Region)>>::new();

    // Idea: pick two regions r1, r2. then shift r2 such that one of its cells neighbors r1.
    for r1 in &rs {
        for r2 in &rs {
            // remember each way we tried shifting r2 and each neighbor we pick
            let mut seen_neighbor = HashSet::new();
            let mut seen_shift = HashSet::new();
            for cell1 in r1 {
                for neighbor in neighbors(cell1) {
                    if !seen_neighbor.insert(neighbor) {
                        continue;
                    }

                    for cell2 in r2 {
                        // shift r2 so that cell2 overlaps with neighbor
                        let offset = (neighbor.0 - cell2.0, neighbor.1 - cell2.1);
                        if seen_shift.insert(offset) {
                            let shifted = shift(offset, r2);
                            if shifted.iter().any(|n| *n < (0, 0)) {
                                // if the shifted region has a node < (0, 0),
                                // then we can skip this recomb because it is
                                // inaccessible by later algorithms (they always
                                // use a region that starts at the origin)
                                continue;
                            }

                            let new_r = &shifted | r1;
                            if new_r.len() == 2 * n {
                                // there are no collisions, this is a valid combination
                                //
                                // note: we need to insert only one of the pairs because the
                                // regions will be renumbered for normalization later.
                                result
                                    .entry(new_r)
                                    .or_default()
                                    .insert((r1.clone(), shifted));
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct Color(pub NonZeroU8);
impl Color {
    fn increment(&mut self) {
        self.0 = self.0.checked_add(1).unwrap();
    }

    pub const fn new(c: u8) -> Self {
        Color(NonZeroU8::new(c).unwrap())
    }
}

/// A graph in the lattice.  This is just a set of nodes.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct Graph {
    nodes: Vec<Node>,
    // an ordinal associated with each node, according to the standard
    // ordering. this allows representing data about nodes using vectors.
    indices: BTreeMap<Node, usize>,
}

impl Graph {
    pub fn from(r: Region) -> Graph {
        let mut nodes = r.inner;
        nodes.sort();
        let indices = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        Graph { nodes, indices }
    }

    // Create a triangle of side length N
    pub fn triangle(n: usize) -> Graph {
        debug_assert!(n > 0);
        let nodes: Vec<Node> = (0..n)
            .flat_map(|i| (0..(n - i)).map(move |j| (i as i8, (i + 2 * j) as i8)))
            .collect();
        let indices = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        Graph { nodes, indices }
    }

    fn node_at(&self, i: usize) -> Node {
        self.nodes[i]
    }

    pub fn nodes(&self) -> &Vec<Node> {
        &self.nodes
    }

    pub fn contains(&self, n: &Node) -> bool {
        self.indices.contains_key(n)
    }

    fn get_index(&self, n: &Node) -> Option<usize> {
        self.indices.get(n).copied()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn into_region(self) -> Region {
        Region::from(self.nodes)
    }
}

/// Tiling of a graph.  We borrow the graph to avoid copying it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tiling<'graph> {
    pub graph: &'graph Graph,
    // color of each node, represented as a 2D array, so we might generate
    // unused slots in this list but it makes lookups faster.
    color: Vec<Option<Color>>,
    next_uncolored_cell: Option<Node>,
    next_color: Color,
}

impl<'g> Tiling<'g> {
    fn of_graph(graph: &'g Graph) -> Tiling<'g> {
        Tiling {
            graph,
            color: vec![None; graph.nodes.len()],
            next_uncolored_cell: Some(graph.nodes[0]),
            next_color: Color(NonZeroU8::new(1).unwrap()),
        }
    }

    fn len(&self) -> usize {
        self.color.len()
    }

    fn set_color(&mut self, n: Node, color: Color) {
        self.color[self.graph.indices[&n]] = Some(color);
        if self.next_uncolored_cell == Some(n) {
            // find the next colored cell
            self.next_uncolored_cell = None;

            for i in self.graph.indices[&n]..self.graph.nodes.len() {
                if self.color[i].is_none() {
                    self.next_uncolored_cell = Some(self.graph.node_at(i));
                    // println!("\tinserted {n:?}, next uncolored cell: {:?}", self.next_uncolored_cell);
                    return;
                }
            }
        }
    }

    pub fn color(&self, n: &Node) -> Option<Color> {
        self.color[self.graph.get_index(n)?]
    }

    // A stable *non-contiguous* selection of colors for rendering
    pub fn color_for_rendering(&self, n: &Node) -> Option<Color> {
        self.color(n).map(|c| {
            Color(
                NonZeroU8::new(
                    self.color
                        .iter()
                        .enumerate()
                        .find(|(_, c_other)| **c_other == Some(c))
                        .unwrap()
                        .0 as u8
                        + 1,
                )
                .unwrap(),
            )
        })
    }

    // renumber the colors so that permutations of colors are mapped to the same coloring. we do
    // this by assigning numbers to colors in the order they appear when traversing the graph
    // top-bottom, left-to-right.
    fn normalize(&mut self) {
        let mut coloring = HashMap::<Color, Color>::new();
        let mut next_color = Color(NonZeroU8::new(1).unwrap());

        for c in self.color.iter_mut().flatten() {
            *c = *coloring.entry(*c).or_insert_with(|| {
                let c = next_color;
                next_color.increment();
                c
            });
        }

        // todo: sanity check for colorings?
    }

    // try coloring a new region at given node only if it would not collide with existing regions
    fn try_insert<'a, I: IntoIterator<Item = &'a Node>>(
        mut self,
        (x0, y0): Node,
        r: I,
    ) -> Option<Tiling<'g>> {
        for (x, y) in r {
            let node = (x0 + *x, y0 + *y);

            // Check that this node is in-bounds and uncolored.
            // TODO(maemre): validate this reasoning for bounds checking, extend to the "soft region"
            if (!self.graph.contains(&node)) || self.color(&node).is_some() {
                return None;
            }
            self.set_color(node, self.next_color);
        }
        self.next_color.increment();
        Some(self)
    }

    // Paint given region to the given color (it may originally span multiple colors)
    fn paint(&mut self, r: &Region, color: Color) {
        for node in r {
            self.set_color(*node, color);
        }
    }

    // Enumerate all partial colorings of the given graph with given tile size
    pub fn enumerate(g: &'g Graph, r: usize) -> HashSet<Tiling<'g>> {
        assert!(!g.nodes.is_empty());
        assert!(r > 0);

        let mut visited = HashSet::new();
        let mut worklist = vec![Tiling::of_graph(g)];

        let regions = regions(r);

        while let Some(tiling) = worklist.pop() {
            if !visited.insert(tiling.clone()) {
                continue;
            }

            // println!("processing\n{g}");
            if let Some(n) = tiling.next_uncolored_cell {
                // println!("next uncolored cell: {n:?}");
                for r in &regions {
                    // println!("trying {r:?}");
                    if let Some(mut g_new) = tiling.clone().try_insert(n, r) {
                        // println!("generated:\n{g_new}");
                        g_new.normalize();
                        // println!("normalized:\n{g_new}");
                        worklist.push(g_new);
                    }
                }
            }
        }

        visited
    }

    /// Enumerate all *minimal* covers of the given graph that:
    ///
    /// 1. use tiles of given size, and
    /// 2. are contained in the given extension graph.
    ///
    /// Requirements:
    /// - g must be non-empty.
    /// - extension must contain g.
    /// - tile_size must be positive.
    ///
    /// This function returns a set of regions because we deliberately forget
    /// different tilings for the same region.
    ///
    /// This function returns None if one of the covers matches a known
    /// counterexample.
    pub fn min_covers(
        g: &'g Graph,
        allowed_in_covers: &'g Graph,
        tile_size: usize,
        counterexamples: &HashSet<Region>,
    ) -> Option<HashSet<CompactRegion>> {
        assert!(!g.nodes.is_empty());
        assert!(tile_size > 0);

        assert!(
            g.nodes.iter().all(|n| allowed_in_covers.contains(n)),
            "the extension must be a superset of the graph"
        );

        debug!("{:?}", g);
        debug!("{:?}", allowed_in_covers);

        // we don't care about the actual tilings so we can work on regions
        let mut visited = HashSet::new();
        let mut worklist = vec![CompactRegion::empty()];

        let tiles = regions(tile_size);

        while let Some(compact_region) = worklist.pop() {
            if !visited.insert(compact_region) {
                continue;
            }
            let region = compact_region.to_region(allowed_in_covers);
            debug!("popped {region:?}");

            // the first check is for performance
            let fully_covered = || g.nodes.iter().all(|n| region.contains(n));
            if region.len() >= g.len() && counterexamples.contains(&region) && fully_covered() {
                return None;
            }

            let neighbors = region
                .neighbors()
                .unwrap_or_else(|| Set::from([*g.indices.first_key_value().unwrap().0]));

            debug!("{:#?}", neighbors);

            // consider only the neighbors in the graph, otherwise the added
            // tile is not guaranteed to be in a minimal cover.
            for n in neighbors.into_iter().filter(|n| g.contains(n)) {
                debug!("considering neighbor {n:?}");
                for t in &tiles {
                    // should identify each point in the tile with the neighbor
                    for n_t in &t.inner {
                        let shifted = shift(sub(n, *n_t), t);
                        debug!("got tile {t:?} -> {shifted:?}");

                        // add only valid tiles that:
                        // - do not intersect with the region, and
                        // - are contained in the extension
                        if shifted
                            .iter()
                            .all(|n| (!region.contains(n)) && allowed_in_covers.contains(n))
                        {
                            let combined = &region | &shifted;
                            debug!("new region: {combined:?}");
                            let compact = CompactRegion::from(&combined, allowed_in_covers);
                            if !visited.contains(&compact) {
                                worklist.push(compact);
                            }
                        }
                    }
                }
            }
        }

        visited.retain(|cover| g.nodes.iter().all(|n| cover.contains(n, allowed_in_covers)));
        Some(visited)
    }

    /// Parallel version of [`Self::min_covers`] using the `Worklist` work-stealing infrastructure.
    pub fn par_min_covers(
        g: &'g Graph,
        allowed_in_covers: &'g Graph,
        tile_size: usize,
    ) -> Option<HashSet<CompactRegion>> {
        use crate::concurrency::{Task, WithCost, Worklist};
        use std::sync::atomic::Ordering;
        use std::thread;

        assert!(!g.nodes.is_empty());
        assert!(tile_size > 0);
        assert!(
            g.nodes.iter().all(|n| allowed_in_covers.contains(n)),
            "the extension must be a superset of the graph"
        );

        let tiles = regions(tile_size);
        let n_threads = rayon::current_num_threads();

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
                for t in &tiles {
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

        let g_compact = CompactRegion::from(&g.clone().into_region(), allowed_in_covers);

        let worklist = Worklist::new(
            [WithCost(CompactRegion::empty(), 0)],
            n_threads,
            allowed_in_covers.len(),
            true,
        );
        let last_cleanup = AtomicUsize::new(0);
        let processed = AtomicUsize::new(0);

        thread::scope(|s| {
            for _ in 0..n_threads {
                s.spawn(|| {
                    loop {
                        let WithCost(compact_region, _) = match worklist.pop() {
                            Task::Done => return,
                            Task::Todo(x) => x,
                        };

                        let size = compact_region.len();

                        if size > last_cleanup.fetch_max(size, Ordering::SeqCst) {
                            log::info!("cleaning up regions up to size {size}");
                            worklist.retain_up_to_max_cost(size, |cr| cr.len() >= g_compact.len() && cr.is_superset_of(g_compact));
                        }

                        let processed = processed.fetch_add(1, Ordering::SeqCst);
                        if processed.is_multiple_of(1_000_000) {
                            let seen_sizes = worklist.seen.iter().map(|s| s.len()).enumerate().collect::<BTreeMap<_, _>>();
                            log::info!("processed {processed} items off the queue, retained seen set sizes: {seen_sizes:?}");
                        }

                        // Compute the frontier as a bitmask.
                        //
                        // frontier = neighbors of the current region that are
                        // in g but not yet in the region.
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

                        // Iterate frontier bits and look up precomputed tile masks.
                        let mut children = vec![];
                        let mut frontier = frontier_mask;
                        while frontier != 0 {
                            let ai = frontier.trailing_zeros() as usize;
                            frontier &= frontier - 1;
                            let gi = allowed_idx_to_g_idx[ai];
                            for &tile_mask in &tiles_for_g_node[gi] {
                                if compact_region.0 & tile_mask == 0 {
                                    let combined = CompactRegion(compact_region.0 | tile_mask);
                                    let combined_size = combined.len();
                                    if !worklist.seen[combined_size].contains_sync(&combined) {
                                        children.push(WithCost(combined, combined_size as isize));
                                    }
                                }
                            }
                        }
                        worklist.push_all(children);
                    }
                });
            }
        });

        let mut result = HashSet::new();
        for set in &worklist.seen {
            set.iter_sync(|cr: &CompactRegion| {
                if cr.is_superset_of(g_compact) {
                    result.insert(*cr);
                }
                true
            });
        }
        Some(result)
    }

    // Recombine given two colors (the corresponding regions must be adjacent)
    fn try_recombine(
        &self,
        recomb: &BTreeMap<Region, Set<(Region, Region)>>,
        r1: &Region,
        r2: &Region,
    ) -> Vec<Tiling<'g>> {
        // Get the colors
        let c1 = self[*r1.first().unwrap()].unwrap();
        let c2 = self[*r2.first().unwrap()].unwrap();

        // Move the regions so that the beginning of r1 is at the origin
        let origin = *r1.first().unwrap();
        let mut combined = shift(flip(origin), r1);
        combined.append(&mut shift(flip(origin), r2));

        if let Some(new_splits) = recomb.get(&combined) {
            new_splits
                .iter()
                .map(|(r1, r2)| {
                    let mut g = self.clone();
                    g.paint(&shift(origin, r1), c1);
                    g.paint(&shift(origin, r2), c2);
                    g.normalize();
                    g
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Neighbors of this tiling in the metagraph built via recombinations
    pub fn neighbors(
        &self,
        recomb: &BTreeMap<Region, Set<(Region, Region)>>,
    ) -> HashSet<Tiling<'g>> {
        let n_colors = self.next_color.0.get() as usize - 1;

        let mut regions: Vec<Region> = vec![Region::new(); n_colors];

        for (j, c) in self.color.iter().enumerate() {
            regions[c.unwrap().0.get() as usize - 1].insert(self.graph.node_at(j));
        }

        // Select a pair of regions up to ordering
        regions
            .iter()
            .enumerate()
            .par_bridge()
            .flat_map(|(i, r1)| {
                regions[..i]
                    .iter()
                    .flat_map(|r2| self.try_recombine(recomb, r2, r1))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    // Find the graphs with region size `k` reachable from this graph by recombining graphs
    pub fn reachable(&self, k: usize) -> HashSet<Tiling<'g>> {
        let n_colors = self.next_color.0.get() as usize - 1;

        assert_eq!(
            n_colors * k,
            self.len(),
            "n_colors = {n_colors}, n_tiles = {}, k = {k}\n{self}",
            self.len()
        );
        let mut visited = HashSet::new();
        let mut worklist = vec![self.clone()];
        let recomb = recomb(k);
        debug_only! {
                for (r1, rs) in &recomb {
            eprintln!("{r1:?}: {rs:?}");
                }
        }

        while let Some(g) = worklist.pop() {
            if !visited.insert(g.clone()) {
                continue;
            }

            // Extract regions
            let mut regions: Vec<Region> = vec![Region::new(); n_colors];

            for (j, c) in g.color.iter().enumerate() {
                regions[c.unwrap().0.get() as usize - 1].insert(g.graph.node_at(j));
            }

            // Select a pair of regions up to ordering
            worklist.par_extend(regions.iter().enumerate().par_bridge().flat_map(|(i, r1)| {
                regions[..i]
                    .iter()
                    .flat_map(|r2| g.try_recombine(&recomb, r2, r1))
                    .filter(|g_new| !visited.contains(g_new))
                    .collect::<Vec<_>>()
            }));
            // for (i, r1) in  {
            //     for r2 in &regions[..i] {
            //         for g_new in g.try_recombine(&recomb, r2, r1) {
            //             if !visited.contains(&g_new) {
            //                 worklist.push(g_new);
            //             }
            //         }
            //     }
            // }
        }

        visited
    }

    pub fn is_complete(&self) -> bool {
        self.next_uncolored_cell.is_none()
    }
}

// Flip the sign of given node
fn flip(node: Node) -> Node {
    (-node.0, -node.1)
}

impl<'g> Index<Node> for Tiling<'g> {
    type Output = Option<Color>;
    fn index(&self, n: Node) -> &Self::Output {
        // eprintln!("node: {n:?}");
        // eprintln!("graph: {:?}", self.graph);
        // eprintln!("{}", self.graph.indices[&n]);
        // eprintln!("{}", self.color.len());
        &self.color[self.graph.indices[&n]]
    }
}
