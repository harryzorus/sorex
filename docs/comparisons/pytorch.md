# PyTorch Search Library Comparison

**Date:** 2026-01-19T18:20:20.014Z
**Platform:** darwin, aarch64

## Search Latency

**Key Difference - Progressive vs Batch Results:**

- **Sorex**: Returns results progressively as each tier completes
- **Other libraries**: Return all results at once

## Exact Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| tensor | **Sorex** | 55.4 | 91.7 | 491.3 | 248+10+0 |
| tensor | FlexSearch | - | - | 12.9 | 217 |
| tensor | MiniSearch | - | - | 276.8 | 258 |
| tensor | lunr.js | - | - | 236.2 | 249 |
| tensor | fuse.js | - | - | 30341.3 | 261 |
| module | **Sorex** | 29.7 | 29.7 | 471.0 | 70+0+31 |
| module | FlexSearch | - | - | 1.3 | 90 |
| module | MiniSearch | - | - | 44.5 | 74 |
| module | lunr.js | - | - | 33.5 | 67 |
| module | fuse.js | - | - | 34400.3 | 82 |
| forward | **Sorex** | 8.5 | 8.5 | 429.2 | 54+0+0 |
| forward | FlexSearch | - | - | 1.0 | 56 |
| forward | MiniSearch | - | - | 33.9 | 55 |
| forward | lunr.js | - | - | 24.1 | 51 |
| forward | fuse.js | - | - | 40356.3 | 68 |
| backward | **Sorex** | 5.7 | 5.7 | 436.3 | 51+0+0 |
| backward | FlexSearch | - | - | 0.9 | 53 |
| backward | MiniSearch | - | - | 112.0 | 57 |
| backward | lunr.js | - | - | 20.2 | 48 |
| backward | fuse.js | - | - | 39741.4 | 63 |
| autograd | **Sorex** | 6.0 | 6.0 | 443.4 | 79+0+0 |
| autograd | FlexSearch | - | - | 1.3 | 93 |
| autograd | MiniSearch | - | - | 123.1 | 80 |
| autograd | lunr.js | - | - | 30.3 | 69 |
| autograd | fuse.js | - | - | 47808.1 | 83 |
| optim | **Sorex** | 9.4 | 21.1 | 393.7 | 21+31+39 |
| optim | FlexSearch | - | - | 0.8 | 24 |
| optim | MiniSearch | - | - | 38.5 | 52 |
| optim | lunr.js | - | - | 20.2 | 48 |
| optim | fuse.js | - | - | 35379.3 | 196 |

## Prefix Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| ten | **Sorex** | 0.0 | 68.9 | 413.5 | 0+259+34 |
| ten | FlexSearch | - | - | 0.5 | 3 |
| ten | MiniSearch | - | - | 121.7 | 266 |
| ten | lunr.js | - | - | 4.1 | 4 |
| ten | fuse.js | - | - | 26383.3 | 271 |
| mod | **Sorex** | 0.0 | 52.6 | 405.0 | 0+105+93 |
| mod | FlexSearch | - | - | 0.6 | 10 |
| mod | MiniSearch | - | - | 70.7 | 115 |
| mod | lunr.js | - | - | 6.1 | 6 |
| mod | fuse.js | - | - | 23702.3 | 114 |
| for | **Sorex** | 0.0 | 21.2 | 381.2 | 0+99+87 |
| for | FlexSearch | - | - | 1.5 | 178 |
| for | MiniSearch | - | - | 107.4 | 246 |
| for | lunr.js | - | - | 1.2 | 0 |
| for | fuse.js | - | - | 23769.8 | 229 |
| back | **Sorex** | 3.6 | 21.5 | 361.5 | 31+51+43 |
| back | FlexSearch | - | - | 0.7 | 31 |
| back | MiniSearch | - | - | 51.0 | 86 |
| back | lunr.js | - | - | 19.3 | 36 |
| back | fuse.js | - | - | 27817.6 | 113 |
| auto | **Sorex** | 3.3 | 22.8 | 386.3 | 10+91+113 |
| auto | FlexSearch | - | - | 0.6 | 10 |
| auto | MiniSearch | - | - | 54.7 | 101 |
| auto | lunr.js | - | - | 6.4 | 7 |
| auto | fuse.js | - | - | 32176.3 | 164 |
| opt | **Sorex** | 0.0 | 40.8 | 397.2 | 0+186+95 |
| opt | FlexSearch | - | - | 0.6 | 8 |
| opt | MiniSearch | - | - | 100.2 | 225 |
| opt | lunr.js | - | - | 4.8 | 6 |
| opt | fuse.js | - | - | 25636.1 | 195 |

## Fuzzy Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| tensro | **Sorex** | 0.0 | 0.0 | 573.5 | 0+0+256 |
| tensro | FlexSearch | - | - | 0.4 | 0 |
| tensro | MiniSearch | - | - | 185.2 | 256 |
| tensro | lunr.js | - | - | 1.4 | 0 |
| tensro | fuse.js | - | - | 55245.2 | 263 |
| modul | **Sorex** | 0.0 | 33.8 | 440.0 | 0+80+26 |
| modul | FlexSearch | - | - | 0.4 | 0 |
| modul | MiniSearch | - | - | 147.6 | 105 |
| modul | lunr.js | - | - | 33.1 | 67 |
| modul | fuse.js | - | - | 31044.9 | 92 |
| forwrd | **Sorex** | 0.0 | 0.0 | 435.2 | 0+0+56 |
| forwrd | FlexSearch | - | - | 0.4 | 0 |
| forwrd | MiniSearch | - | - | 106.6 | 56 |
| forwrd | lunr.js | - | - | 1.4 | 0 |
| forwrd | fuse.js | - | - | 50879.4 | 58 |
| backwrd | **Sorex** | 0.0 | 0.0 | 463.3 | 0+0+73 |
| backwrd | FlexSearch | - | - | 0.4 | 0 |
| backwrd | MiniSearch | - | - | 115.3 | 73 |
| backwrd | lunr.js | - | - | 1.6 | 0 |
| backwrd | fuse.js | - | - | 61544.7 | 81 |
| autogrd | **Sorex** | 0.0 | 0.0 | 458.3 | 0+0+79 |
| autogrd | FlexSearch | - | - | 0.5 | 0 |
| autogrd | MiniSearch | - | - | 124.4 | 79 |
| autogrd | lunr.js | - | - | 1.6 | 0 |
| autogrd | fuse.js | - | - | 64999.0 | 82 |
| optimzer | **Sorex** | 0.0 | 0.0 | 448.3 | 0+0+41 |
| optimzer | FlexSearch | - | - | 0.4 | 0 |
| optimzer | MiniSearch | - | - | 107.5 | 41 |
| optimzer | lunr.js | - | - | 1.4 | 0 |
| optimzer | fuse.js | - | - | 65798.7 | 42 |
