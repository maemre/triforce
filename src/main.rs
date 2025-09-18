use std::{collections::{BTreeSet as Set, *}, iter::Enumerate, num::{NonZero, NonZeroI8, NonZeroU8}, ops::{Index, IndexMut, RangeBounds}};

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
fn regions(n: usize) -> Set<Set<Node>> {
    debug_assert!(n > 0);
    if n == 1 {
        return Set::from([Set::from([(0, 0)])]);
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
                }).collect::<Vec<BTreeSet<_>>>()
        })
        .collect()
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
struct Color(NonZeroU8);
impl Color {
    fn increment(&mut self) {
        self.0 = self.0.checked_add(1).unwrap();
    }
}

// The graph is represented as a 2D array
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        self.color[n.0 as usize][((n.1 - n.0)/2) as usize] = Some(color);
        if self.next_uncolored_cell == Some(n) {
            // find the next colored cell
            self.next_uncolored_cell = None;
            let (x, y) = n;
            
            // search current column
            for (j, color) in self.color[x as usize][(((y - x)/2) as usize)..].iter().enumerate() {
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
        if let Some (u) = &mut self.next_uncolored_cell {
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
    fn try_insert<'a, I: IntoIterator<Item = &'a Node>>(mut self, (x0, y0): Node, r: I) -> Option<Graph> {
        for (x, y) in r {
            let node = (x0 + *x, y0 + *y);
            let j = (node.1 - node.0) / 2;
            if node.0 < 0 || ! (0..(self.len() as isize - node.0)).contains(&j) {
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

    // Enumerate all partial colorings of length `n` with region size `r`
    fn enumerate(n: usize, r: usize) -> HashSet<Graph> {
        assert!(n > 0);
        assert!(r > 0);
        
        let mut visited = HashSet::new();
        let mut worklist = vec![Graph::new(n)];
        
        let regions = regions(r);
        
        while let Some(g) = worklist.pop() {
            if ! visited.insert(g.clone()) {
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
}

impl Index<Node> for Graph {
    type Output = Option<Color>;
    fn index(&self, n: Node) -> &Self::Output {
        &self.color[n.0 as usize][((n.1 - n.0)/2) as usize]
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
                    write!(f, " {}", c.0)?;
                } else {
                    write!(f, " .")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

// Size of the triangle we are considering (a finite triangular section of the
// infinite lattice).
static N: usize = 4;

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
    
    for g in Graph::enumerate(3, 2) {
        println!("{g}\n");
    }
    
    for n in 2..=7 {
        for j in 2..n {
        println!("({n}, {j}): {:>7}", Graph::enumerate(n, j).len());
        }
    }
}
