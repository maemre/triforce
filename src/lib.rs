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

use std::{
    collections::{BTreeSet as Set, *},
    num::NonZeroU8,
    ops::{BitOr, Index},
};

use serde::{Serialize, Deserialize};

mod fmt;
mod macros;
pub mod cli;

#[allow(unused_imports)]
pub use fmt::*;
#[allow(unused_imports)]
use macros::*;

// A node is just a pair of coordinates
type Node = (isize, isize);

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

    pub fn iter(&self) -> std::slice::Iter<'_, (isize, isize)> {
        self.inner.iter()
    }

    pub fn insert(&mut self, new: (isize, isize)) -> bool {
        if self.contains(&new) {
            false
        } else {
            self.inner.push(new);
            self.inner.sort();
            true
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
    /// For an empty region, this method returns the set that contains the
    /// origin.
    pub fn neighbors(&self) -> Set<Node> {
        if self.is_empty() {
            Set::from([(0, 0)])
        } else {
            self.inner.iter().flat_map(neighbors).collect()
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

/// Representation of a region for serialization/deserialization.
#[derive(Serialize, Deserialize)]
pub struct MaybeRegion(pub Vec<Node>);

impl MaybeRegion {
    pub fn to_region(mut self) -> Option<Region> {
	self.0.sort();
	let l = self.0.len();
	self.0.dedup();

	if self.0.len() != l {
	    return None;
	}

	if let Some(n) = self.0.first() {
	    if *n != (0, 0) {
		return None;
	    }
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
pub struct Color(NonZeroU8);
impl Color {
    fn increment(&mut self) {
        self.0 = self.0.checked_add(1).unwrap();
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
	Graph {
	    nodes,
	    indices 
	}
    }

    // Create a triangle of side length N
    pub fn triangle(n: usize) -> Graph {
        debug_assert!(n > 0);
	let nodes: Vec<Node> = (0..n).flat_map(|i| (0..(n - i)).map(move |j| (i as isize, (i + 2 * j) as isize))).collect();
	let indices = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
	Graph {
	    nodes,
	    indices,
	}
    }

    fn node_at(&self, i: usize) -> Node {
	self.nodes[i]
    }

    pub fn nodes(&self) -> &Vec<Node> {
        &self.nodes
    }

    fn contains(&self, n: &Node) -> bool {
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
    graph: &'graph Graph,
    // color of each node, represented as a 2D array, so we might generate
    // unused slots in this list but it makes lookups faster.
    color: Vec<Option<Color>>,
    next_uncolored_cell: Option<Node>,
    next_color: Color,
}

impl<'g> Tiling<'g> {
    fn of_graph(graph: &Graph) -> Tiling {
        Tiling {
	    graph,
            color: vec![None; graph.nodes.len()],
            next_uncolored_cell: Some((0, 0)),
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

    fn get_color(&mut self, n: &Node) -> Option<Color> {
	self.color[self.graph.get_index(n)?]
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
            if (!self.graph.contains(&node)) || self.get_color(&node).is_some() {
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
        assert!(! g.nodes.is_empty());
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
    /// This method returns a set of regions because we deliberately forget
    /// different tilings for the same region.
    pub fn min_covers(g: &'g Graph, extension: &'g Graph, tile_size: usize) -> HashSet<Region> {
        assert!(! g.nodes.is_empty());
	assert!(tile_size > 0);

        assert!(g.nodes.iter().all(|n| extension.contains(n)), "the extension must be a superset of the graph");

        debug!("{:?}", g);
        debug!("{:?}", extension);

        // we don't care about the actual tilings so we can work on regions
        let mut visited = HashSet::new();
        let mut worklist = vec![Region::new()];

        let tiles = regions(tile_size);

        while let Some(region) = worklist.pop() {
            if !visited.insert(region.clone()) {
                continue;
            }

            debug!("{:#?}", region.neighbors());

            // consider only the neighbors in the graph, otherwise the added
            // tile is not guaranteed to be in a minimal cover.
            for n in region.neighbors().into_iter().filter(|n| g.contains(n)) {
                debug!("considering neighbor {n:?}");
                for t in &tiles {
                    let shifted = shift(n, t);
                    debug!("got tile {t:?} -> {shifted:?}");

                    // add only valid tiles that:
                    // - do not intersect with the region, and
                    // - are contained in the extension
                    if shifted.iter().all(|n| (! region.contains(n)) && extension.contains(n)) {
                        let combined = &region | &shifted;
                        debug!("new region: {combined:?}");
                        if ! visited.contains(&combined) {
                            worklist.push(combined);
                        }
                    }
                }
            }

        }

        visited.retain(|cover| g.nodes.iter().all(|n| cover.contains(n)));
        visited
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

	debug_only! {
            eprintln!("r1: {r1:?}");
            eprintln!("r2: {r2:?}");
            eprintln!("combined: {combined:?}");
            eprintln!("self:\n{self}");
            eprintln!("c1: {c1}, c2: {c2}");
	}

        if let Some(new_splits) = recomb.get(&combined) {
            debug!("generated:");
            new_splits
                .iter()
                .map(|(r1, r2)| {
                    let mut g = self.clone();
                    g.paint(&shift(origin, r1), c1);
                    g.paint(&shift(origin, r2), c2);
                    g.normalize();
                    debug!("{g}");
                    g
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    // Find the graphs with region size `k` reachable from this graph by recombining graphs
    pub fn reachable(&self, k: usize) -> HashSet<Tiling<'g>> {
        let n_colors = self.next_color.0.get() as usize - 1;
        let n = self.len();

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
            for (i, r1) in regions.iter().enumerate() {
                for r2 in &regions[..i] {
                    for g_new in g.try_recombine(&recomb, r2, r1) {
                        if !visited.contains(&g_new) {
                            worklist.push(g_new);
                        }
                    }
                }
            }
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
