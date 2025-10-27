# triforce - hexagonal lattice enumerator

# Representation of the lattice:

- Nodes are represented by (x, y) coordinates.
- y is even if x is even.
- y is odd if x is odd.
- x is ordered left-to-right
- y is ordered top-to-bottom

So, the coordinates look like

```

(0,0)     (2,0)
     (1,1)
(0,2)     (2,2)
     (1,3)
(0,4)     (2,4)

```

# Main data structures:

- `Node`: A single graph node, represented as an (x, y) coordinate.
- `Color`: Color (tile id) assigned to a node.
- `Region`: A tile or combination of tiles.  Just a set of nodes.
- `Graph`: A subset of the nodes of the lattice with all edges between them.
  - Formally, this is a set of nodes `V ⊆ V_lattice` along with the edges
    `E_lattice ∩ V × V)` where `V_lattice`, `E_lattice` is the whole lattice.
- `Tiling`: This is a *partial* tiling of a graph.  It is effectively a partial
  coloring where each colored region is contiguous and corresponds to a tile.
  
## Normalization of regions

Nodes in the graph and each region are lexicographically ordered.

Regions are normalized so that the smallest node is the origin.
  
# Usage

The program `exp` runs some assorted experiments on triangles with specific side
lengths.

## Enumerating tilings

The main program `triforce` takes a given graph and tile size, and it calculates
the following:
1. All partial tilings.
2. All complete tilings (by filtering the above).
3. Tilings reachable by the first tiling.

There are two ways to give a graph:
1. One can create a triangle of a given side length.
2. Alternatively, the graph can be given as a list of nodes, provided by a JSON
   file.
   
To see how to use the program in detail, run:

```
cargo run --release --bin triforce -- --help
```

Pass the arguments like below:

```
cargo run --release --bin triforce -- <graph> <tile-size>
```

For example, the command below runs it on a triangle of side-length 5 and tile size of 3.

```
cargo run --release --bin triforce -- triangle=5 3
```


The command below runs it on a graph stored in `fixed-region.json` and tile size of 3.

```
cargo run --release --bin triforce -- file=fixed-region.json 3
```

## Generating coverings

The `gen-covers` program takes 3 arguments:
- A graph to cover (a.k.a., a fixed region).
- Allowed extensions of the graph, this is a second graph.
- A tile size.

Then, it enumerates all *minimal coverings* of the graph using tiles of the given size.

For example, the following command calculates all minimal coverings of a
triangle of side-length 3 using 3-tiles that are contained in a triangle of
side-length 5 where both triangles are hinged at the origin and facing the same
direction.

```
cargo run --release --bin gen-covers -- triangle=3 triangle=5 3
```

## Graph file format

Graphs (fixed regions) are just a list of nodes subject to two conditions:
- There are no duplicates (this is for sanity checking), and
- The smallest node is (0, 0).

For loading from JSON, we use JSON arrays to encode node coordinates and the
list of nodes, so it's just a nested list.  For example, here is the triangle of
side-length 3 in the JSON format:

```
[[0,0],[0,2],[0,4],[1,1],[1,3],[2,2]]
```
