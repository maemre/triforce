# Opt 5: Pruning via popcount lower bound

## Status: TODO

## Problem

The search explores regions that can never be extended to cover all of `g`, wasting work.

## Fix

After computing `compact_region`, check whether the uncovered nodes of `g` can still
possibly be covered given the remaining space in `allowed_in_covers`:

```rust
let g_mask: u128 = /* precomputed */;
let allowed_mask: u128 = /* all bits set for allowed_in_covers */;
let uncovered_g = (g_mask & !compact_region.0).count_ones() as usize;
let available = (allowed_mask & !compact_region.0).count_ones() as usize;
if available < uncovered_g {
    continue; // can't possibly cover g — prune
}
```

A tighter bound (if tile_size > 1): every uncovered g-node needs to be part of a tile of
`tile_size` nodes, all within `available`. So `available >= uncovered_g` is necessary but
not sufficient. Could tighten to `available >= uncovered_g` and rely on other opts for
the rest.

## Expected impact

Prunes dead branches early, reducing total worklist size. Cheap (two bitwise ops + popcount).
Depends on Opt 1 (compact-only loop) for the masks to be readily available.
