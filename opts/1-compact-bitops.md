# Opt 1: Eliminate `to_region` — work entirely on the bitset

## Status: DONE

## Problem

Every dequeued item in `par_min_covers` calls `compact_region.to_region(allowed_in_covers)`,
which iterates all universe nodes and allocates a `Vec<Node>`. The resulting `Region` is only
used for operations that can be done directly on the `u128` bitset:

- `region.len()` → `compact_region.0.count_ones() as usize`
- `region.contains(n)` → check a single bit (already exists as `CompactRegion::contains`)
- `&region | &shifted` → `compact_region.0 | compact_shifted.0`
- `counterexamples.contains(&region)` → only needed on the rare counterexample hit; convert lazily

## Fix

Add helper methods to `CompactRegion`:
- `fn len(self) -> usize` — `self.0.count_ones() as usize`
- `fn bitor(self, other: Self) -> Self` — `CompactRegion(self.0 | other.0)`

Rewrite the `par_min_covers` loop to stay on `CompactRegion` until a `Region` is actually
needed (only for the counterexample check and the final `to_region` in check-covers).

`region.neighbors()` still needs the node coordinates — handled by Opt 2.

## Expected impact

Eliminates one `Vec` allocation and an O(universe) scan per dequeued node.
