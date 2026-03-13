# Performance Results

## Test command (`cover-test.sh`)

```
RAYON_NUM_THREADS=8 cargo run --release --bin search-covers -- triangle=6 triangle=6 triangle=7 2 line-2.json --exact-cover
```

## Baseline (2026-03-08)

| metric | value |
|--------|-------|
| wall time | 14.75 s |
| user time | 16.39 s |
| sys time | 0.80 s |
| CPU | 116% |
| threads | 8 (RAYON_NUM_THREADS=8) |

### Program output

```
exact cover check: true
tried     1 graphs (including counterexample refutations), graphs in queue: 0, done: false
graph: [(0, 0), (0, 2), (0, 4), (0, 6), (0, 8), (0, 10), (1, 1), (1, 3), (1, 5), (1, 7), (1, 9), (2, 2), (2, 4), (2, 6), (2, 8), (3, 3), (3, 5), (3, 7), (4, 4), (4, 6), (5, 5)]
#covers: 63
# tilings tried: 34808
FOUND [(0, 0), (0, 2), (0, 4), (0, 6), (0, 8), (0, 10), (1, 1), (1, 3), (1, 5), (1, 7), (1, 9), (2, 2), (2, 4), (2, 6), (2, 8), (3, 3), (3, 5), (3, 7), (4, 4), (4, 6), (5, 5)]
found [[0,0],[0,2],[0,4],[0,6],[0,8],[0,10],[1,1],[1,3],[1,5],[1,7],[1,9],[2,2],[2,4],[2,6],[2,8],[3,3],[3,5],[3,7],[4,4],[4,6],[5,5]]
```

The program finds the solution on the very first graph tried (the triangle=6 itself), checking 63 covers and 34,808 tilings.
