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

## Check covers

The `check-covers` program takes 4 arguments:
- A graph to cover (a.k.a., a fixed region).
- Allowed extensions of the graph, this is a second graph.
- A tile size.
- An optional partial tiling

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

## Partial tiling file format

A (partial) tiling is just a list of tiles, encoded in JSON, e.g. `[tile1,
tile2, ...]` where `tileN` is the list of nodes in the tile.

For example, the following file describes 2 vertical tiles next to each other:

```
[[[0,0],[0,2]]
,[[1,1],[1,3]]
]
```

## Example graphs and tilings

The repo contains some inputs to run experiments on.  All examples are
contained in `size-2/`:

- 1-lines.json -- 6-lines.json are partial tilings containing vertical tiles
  next to each other that fit the corner of a right-facing triangle.

- 9-clipped.json is the fixed region we use in our inductive argument in the paper.
- 9-clipped-ind.json is the allowed set for the same inductive argument.

Additionally, `size-2/corner/` contains some cases for the bottom corner:

- `5-clipped.json`, `6-clipped.json`, `7.json`, `8.json`, `9-clipped.json` are
  the corner regions we check.  The versions with the `-cover` suffix are the
  allowed sets for each case.
- `full-col-N.json` is the desired partial tiling desired for each corner case.
