# Opt 2: Precompute per-node neighbor bitmasks

## Status: TODO

## Problem

`region.neighbors()` allocates a `BTreeSet<Node>` by iterating every node in the region and
collecting their 6 lattice neighbors. Called on every dequeued item.

## Fix

Before spawning threads, for each node index `i` in `allowed_in_covers`, precompute a
`neighbor_mask[i]: u128` where bit `j` is set iff `allowed_in_covers.nodes[j]` is a lattice
neighbor of `allowed_in_covers.nodes[i]`.

Then the frontier of a `CompactRegion` within `g` is pure bitwise:

```rust
let frontier_mask: u128 = (0..128)
    .filter(|i| compact_region.0 & (1u128 << i) != 0)
    .fold(0u128, |acc, i| acc | neighbor_mask[i])
    & g_mask
    & !compact_region.0;
```

Or more efficiently using `trailing_zeros` to iterate only set bits.

No `BTreeSet` allocation, no coordinate arithmetic per item.

## Expected impact

Eliminates one `BTreeSet` allocation + O(|region| * 6) work per dequeued node.
Depends on Opt 1 being done first (to make the loop compact-only).
