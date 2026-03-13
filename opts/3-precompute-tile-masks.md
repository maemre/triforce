# Opt 3: Precompute tile placements as compact masks

## Status: TODO

## Problem

The inner loop does per-item:
- `shift(sub(n, *n_t), t)` — coordinate arithmetic
- `shifted.iter().all(|n| !region.contains(n) && allowed_in_covers.contains(n))` —
  O(tile_size * |region|) due to `Region::contains` being a linear scan
- `&region | &shifted` — Vec merge, sort, dedup
- `CompactRegion::from(&combined, allowed_in_covers)` — iterates all universe nodes

## Fix

Before spawning threads, for each node `n` in `g`, enumerate all tile placements that cover `n`
and are fully within `allowed_in_covers`. Store each as a precomputed `u128` mask:

```rust
let tiles_for: Vec<Vec<u128>> = g.nodes.iter().map(|n| {
    tiles.iter().flat_map(|t| t.inner.iter().filter_map(|n_t| {
        let shifted = shift(sub(*n, *n_t), t);
        if shifted.iter().all(|m| allowed_in_covers.contains(m)) {
            Some(CompactRegion::from(&shifted_region, allowed_in_covers).0)
        } else { None }
    })).collect()
}).collect();
```

Hot inner loop becomes:
```rust
for tile_mask in &tiles_for[g_index_of(n)] {
    if compact_region.0 & tile_mask == 0 {  // no intersection
        let combined = CompactRegion(compact_region.0 | tile_mask);
        ...
    }
}
```

## Expected impact

Eliminates `shift`, `sub`, O(tile_size * |region|) contains-checks, Vec merge, and
`CompactRegion::from` from the hot loop. Reduces inner body to two integer ops.
Depends on Opts 1 + 2.
