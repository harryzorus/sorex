# NVIDIA CUTLASS Search Library Comparison

**Date:** 2026-01-19T18:19:52.508Z
**Platform:** darwin, aarch64

## Search Latency

**Key Difference - Progressive vs Batch Results:**

- **Sorex**: Returns results progressively as each tier completes
- **Other libraries**: Return all results at once

## Exact Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| gemm | **Sorex** | 6.9 | 6.9 | 194.3 | 36+0+19 |
| gemm | FlexSearch | - | - | 1.5 | 50 |
| gemm | MiniSearch | - | - | 77.3 | 37 |
| gemm | lunr.js | - | - | 31.7 | 32 |
| gemm | fuse.js | - | - | 12575.6 | 43 |
| kernel | **Sorex** | 4.8 | 4.8 | 244.3 | 42+0+0 |
| kernel | FlexSearch | - | - | 1.6 | 51 |
| kernel | MiniSearch | - | - | 49.1 | 45 |
| kernel | lunr.js | - | - | 28.3 | 44 |
| kernel | fuse.js | - | - | 13427.3 | 45 |
| tensor | **Sorex** | 13.1 | 13.1 | 263.2 | 48+0+0 |
| tensor | FlexSearch | - | - | 1.0 | 57 |
| tensor | MiniSearch | - | - | 47.9 | 51 |
| tensor | lunr.js | - | - | 25.5 | 50 |
| tensor | fuse.js | - | - | 13428.0 | 52 |
| warp | **Sorex** | 4.9 | 4.9 | 201.1 | 24+0+29 |
| warp | FlexSearch | - | - | 0.7 | 26 |
| warp | MiniSearch | - | - | 24.1 | 28 |
| warp | lunr.js | - | - | 13.1 | 26 |
| warp | fuse.js | - | - | 13787.8 | 54 |
| cuda | **Sorex** | 3.2 | 3.2 | 218.8 | 42+0+27 |
| cuda | FlexSearch | - | - | 0.9 | 54 |
| cuda | MiniSearch | - | - | 27.3 | 43 |
| cuda | lunr.js | - | - | 20.1 | 40 |
| cuda | fuse.js | - | - | 13956.8 | 53 |
| epilogue | **Sorex** | 2.4 | 2.4 | 265.6 | 18+0+0 |
| epilogue | FlexSearch | - | - | 0.9 | 21 |
| epilogue | MiniSearch | - | - | 69.2 | 19 |
| epilogue | lunr.js | - | - | 11.1 | 19 |
| epilogue | fuse.js | - | - | 21045.8 | 19 |

## Prefix Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| ker | **Sorex** | 0.0 | 9.7 | 189.2 | 0+45+20 |
| ker | FlexSearch | - | - | 0.5 | 0 |
| ker | MiniSearch | - | - | 45.5 | 51 |
| ker | lunr.js | - | - | 1.7 | 0 |
| ker | fuse.js | - | - | 9743.0 | 46 |
| ten | **Sorex** | 0.0 | 15.2 | 208.9 | 0+51+19 |
| ten | FlexSearch | - | - | 0.6 | 1 |
| ten | MiniSearch | - | - | 50.7 | 57 |
| ten | lunr.js | - | - | 4.3 | 2 |
| ten | fuse.js | - | - | 10919.3 | 63 |
| war | **Sorex** | 0.0 | 7.1 | 179.8 | 0+45+17 |
| war | FlexSearch | - | - | 0.7 | 1 |
| war | MiniSearch | - | - | 31.7 | 51 |
| war | lunr.js | - | - | 1.7 | 1 |
| war | fuse.js | - | - | 9691.8 | 54 |
| gem | **Sorex** | 0.0 | 8.9 | 206.3 | 0+37+30 |
| gem | FlexSearch | - | - | 0.5 | 0 |
| gem | MiniSearch | - | - | 35.8 | 53 |
| gem | lunr.js | - | - | 1.1 | 0 |
| gem | fuse.js | - | - | 10333.2 | 42 |
| mat | **Sorex** | 0.0 | 7.1 | 217.7 | 0+45+25 |
| mat | FlexSearch | - | - | 0.5 | 0 |
| mat | MiniSearch | - | - | 40.9 | 62 |
| mat | lunr.js | - | - | 1.2 | 0 |
| mat | fuse.js | - | - | 10217.1 | 58 |
| epi | **Sorex** | 0.0 | 4.2 | 182.6 | 0+13+42 |
| epi | FlexSearch | - | - | 0.5 | 6 |
| epi | MiniSearch | - | - | 25.2 | 33 |
| epi | lunr.js | - | - | 1.2 | 0 |
| epi | fuse.js | - | - | 10700.4 | 25 |
| sync | **Sorex** | 3.3 | 3.3 | 193.1 | 12+0+18 |
| sync | FlexSearch | - | - | 0.6 | 12 |
| sync | MiniSearch | - | - | 22.9 | 21 |
| sync | lunr.js | - | - | 4.7 | 5 |
| sync | fuse.js | - | - | 15010.7 | 38 |

## Fuzzy Queries

| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |
|-------|---------|-------------|-------------|-------------|---------|
| kernal | **Sorex** | 0.0 | 0.0 | 261.1 | 0+0+45 |
| kernal | FlexSearch | - | - | 0.4 | 0 |
| kernal | MiniSearch | - | - | 72.4 | 45 |
| kernal | lunr.js | - | - | 1.4 | 0 |
| kernal | fuse.js | - | - | 21015.5 | 50 |
| tensr | **Sorex** | 0.0 | 0.0 | 257.9 | 0+0+56 |
| tensr | FlexSearch | - | - | 0.5 | 0 |
| tensr | MiniSearch | - | - | 81.3 | 56 |
| tensr | lunr.js | - | - | 1.4 | 0 |
| tensr | fuse.js | - | - | 21944.0 | 55 |
| wrp | **Sorex** | 0.0 | 0.0 | 191.2 | 0+0+65 |
| wrp | FlexSearch | - | - | 0.5 | 0 |
| wrp | MiniSearch | - | - | 17.6 | 27 |
| wrp | lunr.js | - | - | 1.0 | 0 |
| wrp | fuse.js | - | - | 9404.4 | 0 |
| gemn | **Sorex** | 0.0 | 0.0 | 208.1 | 0+0+55 |
| gemn | FlexSearch | - | - | 0.5 | 0 |
| gemn | MiniSearch | - | - | 22.2 | 36 |
| gemn | lunr.js | - | - | 1.0 | 0 |
| gemn | fuse.js | - | - | 20178.9 | 65 |
| matrx | **Sorex** | 0.0 | 0.0 | 225.6 | 0+0+42 |
| matrx | FlexSearch | - | - | 0.5 | 0 |
| matrx | MiniSearch | - | - | 75.1 | 42 |
| matrx | lunr.js | - | - | 1.4 | 0 |
| matrx | fuse.js | - | - | 20842.4 | 35 |
| epilouge | **Sorex** | 0.0 | 0.0 | 269.9 | 0+0+18 |
| epilouge | FlexSearch | - | - | 0.4 | 0 |
| epilouge | MiniSearch | - | - | 64.6 | 18 |
| epilouge | lunr.js | - | - | 1.5 | 0 |
| epilouge | fuse.js | - | - | 31153.9 | 19 |
| syncronize | **Sorex** | 0.0 | 0.0 | 256.7 | 0+0+0 |
| syncronize | FlexSearch | - | - | 0.5 | 0 |
| syncronize | MiniSearch | - | - | 140.8 | 7 |
| syncronize | lunr.js | - | - | 1.5 | 0 |
| syncronize | fuse.js | - | - | 37604.1 | 14 |
