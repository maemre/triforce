use std::{
    collections::{BTreeSet as Set, *},
    fmt::{Debug, Display},
    num::NonZeroU8,
    ops::{BitOr, Index},
};

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            eprintln!($($arg)*);
        }
    };
}

/*
Encoding for a triangular lattice:

- Nodes are represented by (x, y) coordinates.
- y is even if x is even.
- y is odd if x is odd.
- x is ordered left-to-right
- y is ordered top-to-bottom

So, the coordinates look like

(0,0)
     (1,1)
(0,2)     (2,2)
     (1,3)
(0,4)

*/

// A node is just a pair of coordinates
type Node = (isize, isize);

// A region is just a (connected) set of nodes
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
struct Region {
    inner: Vec<Node>,
}

impl Region {
    fn new() -> Self {
        Self::from(vec![])
    }

    fn from(mut inner: Vec<Node>) -> Self {
        inner.sort();
        Region { inner }
    }

    fn contains(&self, n: &Node) -> bool {
        self.inner.contains(n)
    }

    fn iter(&self) -> std::slice::Iter<'_, (isize, isize)> {
        self.inner.iter()
    }

    fn insert(&mut self, new: (isize, isize)) -> bool {
        if self.contains(&new) {
            false
        } else {
            self.inner.push(new);
            self.inner.sort();
            true
        }
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn first(&self) -> Option<&Node> {
        self.inner.get(0)
    }

    fn append(&mut self, r2: &mut Region) {
        self.inner.append(&mut r2.inner);
        self.inner.sort();
        self.inner.dedup();
    }
}

impl Debug for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
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

impl<'a> BitOr for &'a Region {
    type Output = Region;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut r = self.clone();
        r.inner.extend(rhs.inner.iter().cloned());
        r.inner.sort();
        r.inner.dedup();
        r
    }
}

// This returns neighbors in an infinite lattice
fn neighbors(n: &Node) -> [Node; 6] {
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

// Generate all connected regions of size n where the top-left portion is moved
// to the origin.
fn regions(n: usize) -> Set<Region> {
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
fn recomb(n: usize) -> BTreeMap<Region, Set<(Region, Region)>> {
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
                            let new_r = &shifted | r1;
                            if new_r.len() == 2 * n {
                                // there are no collisions, this is a valid combination
                                //
                                // note: we need to insert only one of the pairs because the
                                // regions will be renumbered for normalization later.
                                //
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
struct Color(NonZeroU8);
impl Color {
    fn increment(&mut self) {
        self.0 = self.0.checked_add(1).unwrap();
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

// The graph is represented as a 2D array
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Graph {
    color: Vec<Vec<Option<Color>>>,
    next_uncolored_cell: Option<Node>,
    next_color: Color,
}

impl Graph {
    // Create a triangle of size N
    fn new(n: usize) -> Graph {
        debug_assert!(n > 0);
        Graph {
            color: (0..n).map(|i| vec![None; n - i]).collect(),
            next_uncolored_cell: Some((0, 0)),
            next_color: Color(NonZeroU8::new(1).unwrap()),
        }
    }

    fn len(&self) -> usize {
        self.color.len()
    }

    fn set_color(&mut self, n: Node, color: Color) {
        self.color[n.0 as usize][((n.1 - n.0) / 2) as usize] = Some(color);
        if self.next_uncolored_cell == Some(n) {
            // find the next colored cell
            self.next_uncolored_cell = None;
            let (x, y) = n;

            // search current column
            for (j, color) in self.color[x as usize][(((y - x) / 2) as usize)..]
                .iter()
                .enumerate()
            {
                if color.is_none() {
                    self.next_uncolored_cell = Some((x, 2 * j as isize + y));
                    // println!("\tinserted {n:?}, next uncolored cell: {:?}", self.next_uncolored_cell);
                    return;
                }
            }
            // search the rest of the columns
            for x in x..(self.color.len() as isize) {
                for (j, color) in self.color[x as usize].iter().enumerate() {
                    if color.is_none() {
                        self.next_uncolored_cell = Some((x, 2 * j as isize + x));
                        // println!("\tinserted {n:?}, next uncolored cell: {:?}", self.next_uncolored_cell);
                        return;
                    }
                }
            }
        }
    }

    fn uncolor(&mut self, n: Node) {
        self.color[n.0 as usize][(n.1 - n.0) as usize] = None;
        if let Some(u) = &mut self.next_uncolored_cell {
            // n < *u ?
            if n.0 < u.0 || n.0 == u.0 && n.1 < u.1 {
                *u = n;
            }
        }
    }

    fn set_color_opt(&mut self, n: Node, color: Option<Color>) {
        if let Some(c) = color {
            self.set_color(n, c);
        } else {
            self.uncolor(n);
        }
    }

    // renumber the colors so that permutations of colors are mapped to the same coloring. we do
    // this by assigning numbers to colors in the order they appear when traversing the graph
    // top-bottom, left-to-right.
    fn normalize(&mut self) {
        let mut coloring = HashMap::<Color, Color>::new();
        let mut next_color = Color(NonZeroU8::new(1).unwrap());

        for v in &mut self.color {
            for c in v {
                if let Some(c) = c {
                    *c = *coloring.entry(*c).or_insert_with(|| {
                        let c = next_color;
                        next_color.increment();
                        c
                    });
                }
            }
        }

        // todo: sanity check for colorings?
    }

    // try coloring a new region at given node only if it would not collide with existing regions
    fn try_insert<'a, I: IntoIterator<Item = &'a Node>>(
        mut self,
        (x0, y0): Node,
        r: I,
    ) -> Option<Graph> {
        for (x, y) in r {
            let node = (x0 + *x, y0 + *y);
            let j = (node.1 - node.0) / 2;
            if node.0 < 0 || !(0..(self.len() as isize - node.0)).contains(&j) {
                return None;
            }
            if self[node].is_some() {
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

    // Enumerate all partial colorings of length `n` with region size `r`
    fn enumerate(n: usize, r: usize) -> HashSet<Graph> {
        assert!(n > 0);
        assert!(r > 0);

        let mut visited = HashSet::new();
        let mut worklist = vec![Graph::new(n)];

        let regions = regions(r);

        while let Some(g) = worklist.pop() {
            if !visited.insert(g.clone()) {
                continue;
            }

            // println!("processing\n{g}");
            if let Some(n) = g.next_uncolored_cell {
                // println!("next uncolored cell: {n:?}");
                for r in &regions {
                    // println!("trying {r:?}");
                    if let Some(mut g_new) = g.clone().try_insert(n, r) {
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

    // Recombine given two colors (the corresponding regions must be adjacent)
    fn try_recombine(
        &self,
        recomb: &BTreeMap<Region, Set<(Region, Region)>>,
        r1: &Region,
        r2: &Region,
    ) -> Vec<Graph> {
        // Get the colors
        let c1 = self[*r1.first().unwrap()].unwrap();
        let c2 = self[*r2.first().unwrap()].unwrap();

        // Move the regions so that the beginning of r1 is at the origin
        let origin = *r1.first().unwrap();
        let mut combined = shift(flip(origin), r1);
        combined.append(&mut shift(flip(origin), r2));

        debug!("r1: {r1:?}");
        debug!("r2: {r2:?}");
        debug!("combined: {combined:?}");
        debug!("self:\n{self}");
        debug!("c1: {c1}, c2: {c2}");

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
    fn reachable(&self, k: usize) -> HashSet<Graph> {
        let n_colors = self.next_color.0.get() as usize - 1;
        let n = self.len();

        assert_eq!(
            n_colors * k,
            n * (n + 1) / 2,
            "n_colors = {n_colors}, n = {}, k = {k}\n{self}",
            self.len()
        );
        let mut visited = HashSet::new();
        let mut worklist = vec![self.clone()];
        let recomb = recomb(k);
        for (r1, rs) in &recomb {
            debug!("{r1:?}: {rs:?}");
        }

        while let Some(g) = worklist.pop() {
            if !visited.insert(g.clone()) {
                continue;
            }

            // Extract regions
            let mut regions: Vec<Region> = vec![Region::new(); n_colors];

            for (x, v) in g.color.iter().enumerate() {
                for (j, c) in v.iter().enumerate() {
                    regions[c.unwrap().0.get() as usize - 1]
                        .insert((x as isize, (2 * j + x) as isize));
                }
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
}

// Flip the sign of given node
fn flip(node: Node) -> Node {
    (-node.0, -node.1)
}

impl Index<Node> for Graph {
    type Output = Option<Color>;
    fn index(&self, n: Node) -> &Self::Output {
        &self.color[n.0 as usize][((n.1 - n.0) / 2) as usize]
    }
}

impl std::fmt::Display for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        /*
        for line in 0..self.len() {
            for (col, v) in self.color[0..=line].iter().enumerate() {
                if line % 2 != col % 2 {
                    write!(f, " ")?;
                } else if let Some(c) = v[(line - col) / 2] {
                    write!(f, "{}", c.0)?;
                } else {
                    write!(f, ".")?;
                }
            }
            writeln!(f)?;
        }
        if self.len() > 1 {
            // second half
            for i in self.len()..(2 * self.len() - 1) {
                let line = 2 * self.len() - 2 - i;
                for (col, v) in self.color[0..=line].iter().enumerate() {
                    if i % 2 != col % 2 {
                        write!(f, " ")?;
                    } else if let Some(c) = v[(line - col) / 2] {
                        write!(f, "{}", c.0)?;
                    } else {
                        write!(f, ".")?;
                    }
                }
                writeln!(f)?;
            }
        } */
        for (i, v) in self.color.iter().enumerate() {
            write!(f, "{}", " ".repeat(i))?;
            for color in v {
                if let Some(c) = color {
                    write!(f, " {:x}", c.0)?;
                } else {
                    write!(f, " .")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

fn main() {
    let k = 3;
    // list regions of size k
    for r in regions(k) {
        let mut v = r.iter().collect::<Vec<_>>();
        v.sort();
        println!("{v:?}");
    }

    let g = Graph::new(4);
    println!("{g:?}");
    println!("{g}");

    for g in Graph::enumerate(3, 3) {
        println!("{g}\n");
    }

    println!("regions:");
    for k in 2..7 {
        println!("{k}: {:>7}", regions(k).len());
    }

    println!();

    println!("tilings:");
    println!("(n, k):  partial complete   recomb");
    println!("----------------------------------");
    for n in 3..9 {
        for k in 2..6 {
            // for k in 2..(n * (n + 1) / 2) {
            if (n * (n + 1) / 2) % k != 0 {
                continue;
            }
            // println!("{k}");
            let gs = Graph::enumerate(n, k);
            let complete = gs
                .iter()
                .filter(|g| g.next_uncolored_cell.is_none())
                .collect::<Vec<_>>();
            let first = (*complete.iter().min().unwrap()).clone();
            let gs_len = gs.len();
            let complete_len = complete.len();
            drop(complete);
            drop(gs);
            // println!("{}", first);
            let reachable = first.reachable(k);
            println!(
                "({n}, {k}): {:>8} {:>8} {:>8}",
                gs_len,
                complete_len,
                reachable.len()
            );
            // println!("enumerated graphs:");
            // for g in complete {
            //     println!("{g}");
            // }
            // println!("reachable graphs:");
            // for g in reachable {
            //     println!("{g}");
            // }
        }
    }
}
