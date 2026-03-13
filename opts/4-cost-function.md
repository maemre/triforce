# Opt 4: Better cost function (work ordering)

## Status: TODO

## Problem

All worklist items have cost 0 — effectively unordered BFS. The search finds covers in
arbitrary order without bias toward promising regions.

## Fix

Prefer regions that already cover more nodes of `g`. Worklist is a max-heap by cost, so:

```rust
let g_mask: u128 = /* precomputed bitmask of g within allowed */;
let cost = (compact_region.0 & g_mask).count_ones() as isize;
children.push(WithCost(combined, cost));
```

Regions closer to covering `g` are explored first, finding full covers sooner and
enabling more pruning of the seen set.

## Expected impact

Reduces total nodes visited (fewer dead-end partial regions explored). Synergizes with Opt 5.
