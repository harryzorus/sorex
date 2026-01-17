# Sorex - Agent Instructions

**STOP. READ THIS BEFORE MODIFYING ANY CODE.**

This crate has formal verification. We've mathematically proven our search algorithms are correct. If you break an invariant, the proofs won't save you. They'll just tell you exactly how wrong you were.

## The Golden Rules

1. **Run verification after EVERY change**: `cargo xtask verify`
2. **Never modify constants** without updating Lean proofs (the math doesn't lie)
3. **Never bypass type-level wrappers** (`ValidatedSuffixEntry`, `SortedSuffixArray`, `WellFormedIndex`)
4. **Never silence contract violations** they're not suggestions, they're the law

## Quick Reference

```
cargo xtask verify  → Full verification (11 steps: Lean + tests + mutations + E2E)
cargo xtask check   → Quick check (tests + clippy, no Lean/mutations)
cargo xtask test    → Run all tests
cargo xtask lean    → Build Lean proofs only
cargo xtask kani    → Kani model checking (slow, ~5 min, run after major changes)
cargo xtask bench   → Run benchmarks
```

**After major changes to binary parsing, also run Kani:**
```bash
cargo xtask kani    # Proves panic-freedom for all inputs
```

```
INVARIANT VIOLATED? → Your code is wrong, not the check
TEST FAILING?       → Read the Lean spec, understand WHY
LEAN WON'T BUILD?   → You changed something that breaks proofs
```

## Before You Start

```bash
# Verify the codebase is healthy
cargo xtask verify
```

If this fails, **stop and fix it first**.

## Modifying Code

### Safe Changes (Low Risk)

- Adding new tests
- Improving error messages
- Adding logging/metrics
- Documentation changes

### Dangerous Changes (Require Verification)

- ANY change to `types.rs` → Update Lean `Types.lean`
- ANY change to `scoring.rs` → Verify `Scoring.lean` theorems still hold
- ANY change to `index.rs` → Run full test suite + contract checks
- ANY change to `search.rs` → Verify binary search properties
- ANY change to `levenshtein.rs` → Check edit distance bounds

### Forbidden Changes (Will Break Everything)

- Changing scoring constants without Lean proof update
- Bypassing `ValidatedSuffixEntry::new()` validation
- Creating `SuffixEntry` without bounds checking
- Modifying sort comparator in `build_index`
- Changing LCP calculation without updating `check_lcp_correct`

## Invariants You Must Preserve

### 1. Suffix Entry Well-Formedness

```
∀ entry ∈ suffix_array:
  entry.doc_id < texts.len() ∧
  entry.offset < texts[entry.doc_id].len()
```

**Enforced by**: `ValidatedSuffixEntry`, `check_suffix_entry_valid`
**Note**: Strict inequality (`<`) because suffix arrays index non-empty suffixes

### 2. Suffix Array Sortedness

```
∀ i < j:
  suffix_at(suffix_array[i]) ≤ suffix_at(suffix_array[j])
```

**Enforced by**: `SortedSuffixArray`, `check_suffix_array_sorted`
**Why it matters**: Binary search correctness depends on this

### 3. Field Type Dominance

```
Title_score - max_boost > Heading_score + max_boost
Heading_score - max_boost > Content_score + max_boost
```

**Enforced by**: `check_field_hierarchy`, Lean `title_beats_heading` theorem
**Why it matters**: Search ranking must respect field importance

### 4. LCP Correctness

```
lcp[0] = 0
∀ i > 0: lcp[i] = common_prefix_len(suffix_array[i-1], suffix_array[i])
```

**Enforced by**: `check_lcp_correct`

### 5. Index Well-Formedness

```
docs.len() = texts.len()
lcp.len() = suffix_array.len()
∀ boundary: boundary.doc_id < texts.len()
```

**Enforced by**: `WellFormedIndex`, `check_index_well_formed`

### 6. Section Non-Overlap

```
∀ s1 s2 ∈ sections, s1 ≠ s2:
  s1.end_offset ≤ s2.start_offset ∨
  s2.end_offset ≤ s1.start_offset
```

**Enforced by**: `check_sections_non_overlapping`, Lean `offset_maps_to_unique_section`
**Why it matters**: Each text offset must map to exactly one section for correct deep linking

### 7. Section ID Validity

```
∀ section_id:
  section_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
```

**Enforced by**: Property test `prop_section_ids_valid`
**Why it matters**: Section IDs become URL anchors; invalid chars break navigation

## Verification Checklist

Before committing, run the unified verification suite:

```bash
cargo xtask verify
```

This runs all 9 verification steps in the correct order:

| # | Step          | What it checks                              |
|---|---------------|---------------------------------------------|
| 1 | Lean Proofs   | Mathematical specifications compile         |
| 2 | Constants     | Rust/Lean scoring constants are aligned     |
| 3 | Invariants    | Source code has required INVARIANT markers  |
| 4 | Clippy        | No lint warnings (fast, before slow builds) |
| 5 | Release Build | Binary compiles with embedded WASM          |
| 6 | Test Fixtures | E2E test index builds successfully          |
| 7 | Rust Tests    | Unit, integration, and property tests pass  |
| 8 | WASM Parity   | Native and WASM produce identical results   |
| 9 | Browser E2E   | Playwright tests pass in real browser       |

**Why this order?** Proofs first (math foundation), then fast checks (clippy) before slow builds.

## Source Structure

```
src/
├── binary/       # Binary format encoding/decoding
├── build/        # Index building, parallel loading
├── cli/          # CLI (display, mod)
├── fuzzy/        # Levenshtein DFA, edit distance
├── index/        # Suffix array, FST, inverted index
├── runtime/      # Deno/WASM runtime support
├── scoring/      # Ranking (core, ranking)
├── search/       # Tiered search, dedup, union
├── util/         # SIMD, normalization, compression
├── verify/       # Type wrappers, runtime contracts
├── lib.rs        # Library entry point
├── main.rs       # CLI entry point
└── types.rs      # Core data structures
```

```
lean/SearchVerified/
├── Types.lean         # Data structure definitions
├── Scoring.lean       # Field hierarchy, MatchType ranking
├── SuffixArray.lean   # Sorting, completeness, LCP
├── BinarySearch.lean  # Search bounds and correctness
├── Levenshtein.lean   # Edit distance properties
├── Section.lean       # Deep linking, non-overlap
├── TieredSearch.lean  # Three-tier architecture
├── Binary.lean        # Format roundtrip specs
├── Streaming.lean     # Deduplication specs
└── InvertedIndex.lean # Posting list specs
```

## File-by-File Guide

### `src/types.rs`

- **Lean spec**: `Types.lean`
- **Can modify**: Field names, add new fields
- **Cannot modify**: Core struct shapes without Lean update

### `src/index/` (directory)

- **Lean spec**: `SuffixArray.lean`
- **Key files**: `suffix_array.rs`, `fst.rs`, `inverted.rs`
- **Critical function**: `build_index` - creates suffix array
- **INVARIANT**: Output must be sorted and complete

### `src/search/` (directory)

- **Lean spec**: `BinarySearch.lean`, `TieredSearch.lean`
- **Key files**: `suffix.rs`, `tiered.rs`, `dedup.rs`
- **Critical**: Binary search assumes sorted input
- **INVARIANT**: Results must contain all prefix matches

### `src/scoring/core.rs`

- **Lean spec**: `Scoring.lean`
- **CONSTANTS** (DO NOT CHANGE without updating Lean):
  - `Title = 100.0`
  - `Heading = 10.0`
  - `Content = 1.0`
  - `MaxBoost = 0.5`

### `src/fuzzy/levenshtein.rs`

- **Lean spec**: `Levenshtein.lean`
- **Key files**: `levenshtein.rs`, `dfa.rs`
- **INVARIANT**: `|len(a) - len(b)| ≤ distance(a, b)`

### `src/verify/types.rs`

- Type-level invariant wrappers
- **DO NOT BYPASS** - these prevent bugs at compile time

### `src/verify/contracts.rs`

- Runtime debug assertions
- **DO NOT REMOVE** - these catch bugs in debug builds

## Common Mistakes

We've made all of these. Learn from our suffering.

### ❌ Wrong: Creating unchecked entries

```rust
let entry = SuffixEntry { doc_id: 5, offset: 0 }; // NO!
```

### ✅ Right: Use validated wrapper

```rust
let entry = ValidatedSuffixEntry::new(
    SuffixEntry { doc_id: 0, offset: 2 },
    &texts
)?;
```

### ❌ Wrong: Modifying suffix array after creation

```rust
index.suffix_array.push(new_entry); // NO! Breaks sortedness
```

### ✅ Right: Rebuild the entire index

```rust
let index = build_index(docs, texts, boundaries); // Maintains invariants
```

### ❌ Wrong: Changing scoring without proofs

```rust
FieldType::Title => 50.0,  // NO! Breaks field_type_dominance
```

### ✅ Right: Update Lean first, then Rust

```lean
-- In Scoring.lean, verify: 50 - 5 > 10 + 5
theorem title_beats_heading : ... := by native_decide
```

## When Tests Fail

1. **Read the error message** it tells you which invariant broke
2. **Check the Lean spec** understand the mathematical requirement
3. **Fix your code** the invariant is correct; your code is wrong (this is not a democracy)
4. **Add a regression test** prevent future breakage

## The Verification Stack

Six layers of paranoia, because four was never enough.

```
┌───────────────────────────────────────────────────────┐
│                  LEAN SPECIFICATIONS                  │
│     Mathematical truth. Not opinions. Not vibes.      │
│         If Lean builds, the specs are valid.          │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│              KANI MODEL CHECKING                      │
│    Proves panic-freedom for all inputs. Not some.     │
│   Varint parsing proven safe for ANY byte sequence.   │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                TYPE-LEVEL INVARIANTS                  │
│    ValidatedSuffixEntry, SortedSuffixArray, etc.      │
│      Bugs caught at compile time stay caught.         │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                 RUNTIME CONTRACTS                     │
│  check_suffix_array_sorted, check_index_well_formed   │
│       Debug builds panic. Release builds trust.       │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│           PROPERTY TESTS + ORACLE COMPARISON          │
│    proptest with lean_theorem_* / lean_axiom_*        │
│    Random inputs compared against reference oracles.  │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                  MUTATION TESTING                     │
│    cargo-mutants verifies tests catch real bugs.      │
│         CI enforces 60% detection threshold.          │
└───────────────────────────────────────────────────────┘
```

## Advanced Verification Tools

Beyond Lean proofs and property tests, we use additional verification tools.

### Kani Model Checking

Mathematical proof that malformed input cannot crash the parser.

**Location:** `kani-proofs/` (standalone crate to avoid workspace interference)

**Proofs:**
- `verify_encode_varint_no_panic` - Encoding never panics for any u64
- `verify_decode_varint_no_panic` - Decoding never panics for any byte sequence
- `verify_varint_roundtrip` - `decode(encode(x)) == x`
- `verify_decode_empty_input` - Empty input returns EmptyBuffer error
- `verify_decode_rejects_overlong` - 11+ byte varints rejected

**Running Kani:**
```bash
# Must run OUTSIDE the workspace (workspace causes std library conflicts)
cp -r kani-proofs /tmp/kani-proofs
cd /tmp/kani-proofs
cargo kani
```

**CI:** Runs automatically via `.github/workflows/ci.yml` (isolated environment).

### Mutation Testing

Verifies tests actually catch bugs by systematically mutating code.

**Tool:** `cargo-mutants`

```bash
# Run on binary encoding
cargo mutants --package sorex -- --lib --test-threads=1
```

**CI threshold:** 60% detection rate (fails build if lower).

**Known gaps documented in:** `docs/verification-issues.md`

### Oracle-Based Differential Testing

Simple, obviously-correct reference implementations for comparison.

**Location:** `tests/property/oracles.rs`

**Oracles:**
- `oracle_suffix_array` - O(n² log n) naive sort (trivially correct)
- `oracle_lower_bound` - Linear scan (obviously correct)
- `oracle_levenshtein` - Wagner-Fischer DP (textbook algorithm)
- `oracle_common_prefix_len` - Character comparison
- `oracle_encode_varint` / `oracle_decode_varint` - LEB128 reference

**Usage:** Compare optimized Rust against oracles. Any divergence = bug in optimized code.

```rust
proptest! {
    #[test]
    fn prop_sais_matches_oracle(s in "[a-z]{1,100}") {
        let rust_out = build_suffix_array(&s);
        let oracle_out = oracle_suffix_array(&s);
        assert_eq!(rust_out, oracle_out);
    }
}
```

## Emergency: I Broke Something

Don't panic. The proofs are still true. You just violated them.

1. `git stash` your changes
2. `cargo test` to verify main branch works
3. `git stash pop` and compare what you changed
4. The diff will reveal your crimes against mathematics

## Claude Code Workflow

### Before ANY Code Change

```
1. Read the Lean spec for the file you're modifying
2. Identify which invariants your change could affect
3. If changing constants → Update Lean FIRST, then Rust
4. If adding new logic → Add property test FIRST, then implementation
```

### Decision Tree: What to Check

```
Changing scoring.rs?
  └─→ Read lean/SearchVerified/Scoring.lean
  └─→ Check: Does change affect field_type_dominance?
  └─→ If constants change: Update baseScore in Lean, run `lake build`

Changing search.rs or binary search logic?
  └─→ Read lean/SearchVerified/BinarySearch.lean
  └─→ Check: Does change assume sorted input?
  └─→ Verify: findFirstGe_bounds, findFirstGe_lower_bound

Changing levenshtein.rs?
  └─→ Read lean/SearchVerified/Levenshtein.lean
  └─→ Check: Does early-exit optimization still satisfy length_diff_lower_bound?
  └─→ If fuzzy scoring changes: Verify fuzzyScore_monotone

Changing types.rs?
  └─→ Read lean/SearchVerified/Types.lean
  └─→ Check: Do struct shapes still match?
  └─→ If adding fields: Add to Lean structure definition

Changing section boundaries or section_id generation?
  └─→ Read lean/SearchVerified/Section.lean
  └─→ Check: Does change affect non-overlap invariant?
  └─→ Run fuzzing: cargo +nightly fuzz run section_boundaries
  └─→ If section_id format changes: Run prop_section_ids_valid

Adding a new algorithm?
  └─→ Write Lean spec FIRST (even if just axioms)
  └─→ Add property tests that match the Lean spec
  └─→ Implement in Rust
  └─→ Add runtime contracts in contracts.rs
```

### Refactoring Workflow

When refactoring verified code:

```bash
# 1. Understand current invariants
grep -n "INVARIANT" src/*.rs
cat lean/SearchVerified/*.lean | grep -A2 "theorem\|axiom"

# 2. Make change
# ... edit code ...

# 3. Verify nothing broke
cargo xtask verify

# 4. If Lean fails, the refactoring violated a proven property
# Read the error, understand WHY, fix your code (not the proof)
```

### Adding New Features: Move Fast and Prove Things

Most codebases have "move fast and break things." We have "move fast and prove things." The difference is we know when we've broken something before our users do.

**The Philosophy:** If you can't specify it, you can't test it. If you can't test it, you can't implement it. Therefore: specification → tests → implementation. In that order. No exceptions. We've tried the other order. It's how we got here.

```
┌─────────────────────────────────────────────────────────────────┐
│                    MOVE FAST AND PROVE THINGS                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────┐     "What MUST be true?"                      │
│   │    LEAN     │──────────────────────────────────────┐        │
│   │   (specs)   │     User writes theorem/axiom        │        │
│   └──────┬──────┘                                      │        │
│          │                                             │        │
│          ▼                                             │        │
│   ┌─────────────┐     "Encode spec as executable"      │        │
│   │  PROPERTY   │──────────────────────────────────────┤        │
│   │   TESTS     │     Claude derives tests first       │  ┌───┐ │
│   └──────┬──────┘                                      │  │ V │ │
│          │                                             │  │ E │ │
│          ▼                                             │  │ R │ │
│   ┌─────────────┐     "Find edge cases spec missed"    │  │ I │ │
│   │    FUZZ     │──────────────────────────────────────┤  │ F │ │
│   │   TESTS     │     Random bytes → crashes = bugs    │  │ Y │ │
│   └──────┬──────┘                                      │  │   │ │
│          │                                             │  │ A │ │
│          ▼                                             │  │ T │ │
│   ┌─────────────┐     "Make tests pass"                │  │   │ │
│   │    RUST     │──────────────────────────────────────┤  │ E │ │
│   │ NATIVE IMPL │     Easier to debug than WASM        │  │ A │ │
│   └──────┬──────┘                                      │  │ C │ │
│          │                                             │  │ H │ │
│          ▼                                             │  │   │ │
│   ┌─────────────┐     "Prove WASM = Native"            │  │ S │ │
│   │    WASM     │──────────────────────────────────────┤  │ T │ │
│   │   PARITY    │     Same results, different runtime  │  │ E │ │
│   └──────┬──────┘                                      │  │ P │ │
│          │                                             │  └───┘ │
│          ▼                                             │        │
│   ┌─────────────┐                                      │        │
│   │  E2E TESTS  │     "Does it actually work?"         │        │
│   │ (Playwright)│     Browser tests via Deno           │        │
│   └──────┬──────┘──────────────────────────────────────┘        │
│          │                                                      │
│          ▼                                                      │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │              cargo xtask verify                         │   │
│   │   ┌────────┬────────┬─────────┬────────┬────────────┐   │   │
│   │   │  Lean  │ Native │  WASM   │ Clippy │ Constants  │   │   │
│   │   │ Proofs │ Tests  │ Parity  │        │ Alignment  │   │   │
│   │   └────────┴────────┴─────────┴────────┴────────────┘   │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Roles:**
- **User:** Writes Lean proofs/axioms + provides implementation instructions
- **Claude:** Reads specs → derives tests → implements → verifies (in that order, every time)

### Step 0: User Provides Lean Spec + Instructions

The user provides:
1. A Lean specification (theorem or axiom)
2. Instructions for what the implementation should do
3. Which Rust module(s) are affected

Example:
```
I've added this axiom to lean/SearchVerified/Scoring.lean:

axiom subsection_beats_content :
  baseScore .subsection - maxPositionBoost > baseScore .content + maxPositionBoost

Implement a new MatchType.Subsection variant that scores between Section and Content.
Affected files: src/scoring/core.rs, src/types.rs
```

### Step 1: Read the Lean Spec

Before any code, read the Lean file to understand the invariant:

```bash
cat lean/SearchVerified/Scoring.lean
```

Identify:
- What mathematical property must hold?
- How does it relate to existing invariants?

### Step 2: Derive Property Tests

Create tests that encode the Lean spec:

```rust
// In tests/property.rs

/// Maps to Lean axiom: subsection_beats_content
#[test]
fn lean_axiom_subsection_beats_content() {
    assert!(SUBSECTION_SCORE - MAX_BOOST > CONTENT_SCORE + MAX_BOOST);
}

/// Property test with random inputs
#[test]
fn prop_subsection_beats_content_with_boost() {
    proptest!(|(pos in 0.0..1.0f64)| {
        let sub = SUBSECTION_SCORE + position_boost(pos);
        let con = CONTENT_SCORE + position_boost(1.0 - pos);
        prop_assert!(sub > con);
    });
}
```

**Naming:** `lean_theorem_*` for proven theorems, `lean_axiom_*` for axioms, `prop_*` for property tests.

### Step 3: Add Fuzz Tests (if applicable)

For invariants involving parsing, encoding, or edge cases:

```rust
// In fuzz/fuzz_targets/score_calculation.rs

fuzz_target!(|data: &[u8]| {
    if let Ok((match_type, position)) = parse_input(data) {
        let score = calculate_score(match_type, position);
        if match_type == MatchType::Subsection {
            assert!(score > max_possible_content_score());
        }
    }
});
```

### Step 4: Implement (Native Rust First)

Write minimal code to make tests pass:

```rust
// In src/scoring/core.rs
pub const SUBSECTION_BASE_SCORE: f64 = 5.0;
```

**Priority:** Get native Rust working first. It's easier to debug and validate.

```bash
cargo test                    # Native tests
cargo test --test integration # Integration tests
```

### Step 5: Verify Everything

```bash
cargo xtask verify
```

This single command runs all 11 verification steps:

1. **Lean proofs** - Mathematical specifications compile
2. **Constants** - Rust/Lean scoring constants aligned
3. **Spec drift** - Lean/Rust specs haven't diverged
4. **Invariants** - INVARIANT markers present in source
5. **Clippy** - No lint warnings
6. **Release build** - Binary compiles
7. **Test fixtures** - E2E index builds
8. **Rust tests** - Unit, integration, property tests
9. **WASM parity** - Native = WASM output
10. **Browser E2E** - Playwright tests pass
11. **Mutations** - cargo-mutants detection rate >= 60%

If it passes, you're done. If it fails, it tells you exactly which step broke.

**For changes to binary parsing (`src/binary/`), also run Kani:**
```bash
cargo xtask kani    # ~5 min, proves panic-freedom for all inputs
```

**Why this order matters:**

- Lean specs catch logical errors before code exists
- Property tests define the contract implementation must satisfy
- Native-first is easier to debug than WASM
- WASM parity ensures JS consumers work correctly
- Browser E2E verifies the full experience (WASM loading, search UI, results)
- Mutation testing ensures tests actually catch bugs
- `cargo xtask verify` is the definition of "done"

### Interpreting Verification Failures

| Failure                  | Meaning                    | Action                                       |
| ------------------------ | -------------------------- | -------------------------------------------- |
| `lake build` fails       | Lean proof broken          | Your change violated a mathematical property |
| Contract panic           | Runtime invariant violated | Your code produces invalid data              |
| Property test fails      | Random input found a bug   | Check edge cases in your implementation      |
| Constant alignment fails | Rust/Lean drift            | Update the lagging side to match             |

### Lean Spec Quick Reference

```lean
-- Types.lean: Data structure definitions
structure SuffixEntry where doc_id : Nat; offset : Nat

-- Scoring.lean: Ranking invariants (PROVEN)
theorem title_beats_heading : baseScore .title - maxBoost > baseScore .heading + maxBoost

-- SuffixArray.lean: Sorting and completeness (AXIOMATIZED)
axiom build_produces_sorted : SortedSuffixArray sa

-- BinarySearch.lean: Search correctness (AXIOMATIZED)
axiom findFirstGe_bounds : findFirstGe sa texts target ≤ sa.size

-- Levenshtein.lean: Edit distance properties (PROVEN)
theorem fuzzyScore_monotone : d1 ≤ d2 → fuzzyScore d2 max ≤ fuzzyScore d1 max

-- Section.lean: Deep linking correctness (PROVEN)
theorem offset_maps_to_unique_section : NonOverlapping sections → unique_section

-- TieredSearch.lean: Three-tier architecture (AXIOMATIZED)
axiom tier_completeness : tier1 ∪ tier2 ∪ tier3 = all_results

-- Binary.lean: Format roundtrip (AXIOMATIZED, verified by property tests)
axiom varint_roundtrip : decode (encode x) = x
```

**Legend:** PROVEN = verified in Lean, AXIOMATIZED = verified by property/fuzz tests

### Red Flags That Require Lean Review

Stop and check Lean specs if you catch yourself:

- Changing any numeric constant (the math knows what you did)
- Modifying comparison logic (`<`, `<=`, `>`, `>=`)
- Changing array indexing or bounds
- Modifying sort comparators
- Adding new match arms to scoring
- Bypassing validation wrappers "just this once" (famous last words)

## Build System (CLI & Multi-Index)

### `sorex index` Command

The `index` subcommand reads per-document JSON files and constructs a search index. WASM is embedded in the `.sorex` file by default.

```bash
sorex index --input <dir> --output <dir> [--demo]
```

**Flags:**

- `--input <dir>`: Directory containing `manifest.json` and per-document JSON files
- `--output <dir>`: Output directory for `.sorex` file
- `--demo`: Generate demo HTML page showing integration example

### `sorex inspect` Command

Inspect a `.sorex` file's structure and metadata:

```bash
sorex inspect <file.sorex>
```

### Input Format

```
input/
├── manifest.json                    # Document list
│   {
│     "version": 1,
│     "documents": ["0.json", "1.json", ...]
│   }
├── 0.json                           # Per-document JSON files
├── 1.json
└── ...
```

**Per-document JSON structure:**

```json
{
  "id": 0,
  "slug": "my-post",
  "title": "Post Title",
  "excerpt": "Summary",
  "href": "/posts/2026/01/my-post",
  "type": "post",
  "category": "engineering",
  "author": "John Doe",
  "tags": ["rust", "search"],
  "text": "normalized searchable content",
  "fieldBoundaries": [
    { "start": 0, "end": 10, "fieldType": "title", "sectionId": null },
    { "start": 11, "end": 50, "fieldType": "content", "sectionId": "intro" }
  ]
}
```

### Output Format

```
output/
├── index-{hash}.sorex               # Self-contained binary (v7 with embedded WASM)
├── sorex.js                  # JS loader for browser integration
└── demo.html                        # (if --demo)
```

Each `.sorex` file is self-contained with embedded WASM (~330KB raw, ~153KB gzipped). The `sorex.js` extracts and initializes the WASM runtime.

### Parallel Architecture

1. **Phase 1 (Parallel)**: Load documents
   - Rayon `par_iter` over JSON files
   - Parse each document independently
   - Warn and continue on parse errors

2. **Phase 2 (Parallel)**: Build indexes
   - Rayon `par_iter` over index definitions
   - Each index constructed independently
   - Shared Levenshtein DFA (built once, Arc-shared)
   - Each index uses `build_fst_index()` (verified)
   - WASM bytes embedded in each index

3. **Phase 3 (Sequential)**: Emit files
   - Write `.sorex` files (binary format v7 with WASM)
   - Write `sorex.js`
   - Write `demo.html` (if `--demo`)

### No New Invariants Required

The build system is a **preprocessing layer** above verified functions:

- ✅ Document filtering/remapping: Not part of search correctness
- ✅ Each index construction: Uses existing `build_fst_index()` (verified)
- ✅ WASM embedding: Independent of index logic
- ✅ Runtime validation: Existing `WellFormedIndex` checks catch bugs

**Example: Doc ID Remapping**

```rust
// When filtering documents, doc_ids are remapped (0, 1, 2, ...)
// But FieldBoundary doc_ids must stay synchronized
// This is enforced by existing check_index_well_formed() in build_index()
```

### Code Organization

- `src/cli.rs` - Clap CLI definitions
- `src/build/mod.rs` - Main orchestration
- `src/build/manifest.rs` - Input manifest parsing
- `src/build/document.rs` - Per-document structure
- `src/build/parallel.rs` - Parallel loading and construction
- `tools/` - TypeScript loader and build tools (see below)

### JavaScript Loader

The `sorex.js` file is generated from TypeScript in `tools/`:

```
tools/
├── loader.ts     # Main loader: parsing, WASM init, search API
├── build.ts      # Build orchestration script
├── bench.ts      # Benchmarking utilities
├── crawl.ts      # Web crawling for datasets
└── deno.json     # Deno configuration
```

**To rebuild after modifying loader code:**

```bash
cd tools && deno task build
```

The bundled `sorex.js` is embedded in the Rust CLI via `include_str!` and emitted during `sorex index`.

### Example Usage

```bash
# Build index (WASM embedded by default)
sorex index --input ./search-input --output ./search-output

# Build with demo HTML page
sorex index --input ./search-input --output ./search-output --demo

# Inspect built index
sorex inspect ./search-output/index-*.sorex
```

## More Documentation

- **Binary format**: See `docs/architecture.md`
- **Verification details**: See `docs/verification.md`
- **Verification issues & fixes**: See `docs/verification-issues.md`
- **Verification audit findings**: See `docs/verification-findings.md`
- **Build system**: See this section above
