# Sieve - Agent Instructions

**STOP. READ THIS BEFORE MODIFYING ANY CODE.**

This crate has formal verification. Breaking invariants causes silent data corruption.

## The Golden Rules

1. **Run verification after EVERY change**: `cargo xtask verify`
2. **Never modify constants** without updating Lean proofs
3. **Never bypass type-level wrappers** (`ValidatedSuffixEntry`, `SortedSuffixArray`, `WellFormedIndex`)
4. **Never silence contract violations** - they exist to catch your bugs

## Quick Reference

```
cargo xtask verify  → Full verification (tests + Lean + clippy + constant alignment)
cargo xtask check   → Quick check (tests + clippy, no Lean)
cargo xtask test    → Run all tests
cargo xtask lean    → Build Lean proofs only
cargo xtask bench   → Run benchmarks
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

Before committing, verify ALL of these pass:

```bash
# 1. Rust tests (includes property tests)
cargo test

# 2. Lean proofs
cd lean && lake build && cd ..

# 3. Debug contracts (run tests in debug mode)
cargo test --no-release

# 4. Property tests with more cases
cargo test proptest -- --test-threads=1

# 5. Fuzzing (for section-related code)
cargo +nightly fuzz run section_boundaries -- -max_total_time=60
cargo +nightly fuzz run heading_id_validation -- -max_total_time=60
```

## File-by-File Guide

### `types.rs`
- **Lean spec**: `Types.lean`
- **Can modify**: Field names, add new fields
- **Cannot modify**: Core struct shapes without Lean update

### `index.rs`
- **Lean spec**: `SuffixArray.lean`
- **Critical function**: `build_index` - creates suffix array
- **INVARIANT**: Output must be sorted and complete

### `search.rs`
- **Lean spec**: `BinarySearch.lean`
- **Critical**: Binary search assumes sorted input
- **INVARIANT**: Results must contain all prefix matches

### `scoring.rs`
- **Lean spec**: `Scoring.lean`
- **CONSTANTS** (DO NOT CHANGE without updating Lean):
  - `Title = 100.0`
  - `Heading = 10.0`
  - `Content = 1.0`
  - `MaxBoost = 0.5`

### `levenshtein.rs`
- **Lean spec**: `Levenshtein.lean`
- **INVARIANT**: `|len(a) - len(b)| ≤ distance(a, b)`

### `verified.rs`
- Type-level invariant wrappers
- **DO NOT BYPASS** - these prevent bugs at compile time

### `contracts.rs`
- Runtime debug assertions
- **DO NOT REMOVE** - these catch bugs in debug builds

## Common Mistakes

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

1. **Read the error message** - it tells you which invariant broke
2. **Check the Lean spec** - understand the mathematical requirement
3. **Fix your code** - the invariant is correct, your code is wrong
4. **Add a regression test** - prevent future breakage

## Architecture

```
┌───────────────────────────────────────────────────────┐
│                  LEAN SPECIFICATIONS                  │
│  (Mathematical truth - if Lean builds, specs valid)   │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                TYPE-LEVEL INVARIANTS                  │
│  ValidatedSuffixEntry, SortedSuffixArray, etc.        │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                 RUNTIME CONTRACTS                     │
│  check_suffix_array_sorted, check_index_well_formed   │
└───────────────────────────────────────────────────────┘
                          │
                          ▼
┌───────────────────────────────────────────────────────┐
│                  PROPERTY TESTS                       │
│  proptest with lean_proptest_* functions              │
└───────────────────────────────────────────────────────┘
```

## Emergency: I Broke Something

1. `git stash` your changes
2. `cargo test` to verify main branch works
3. `git stash pop` and compare what you changed
4. The diff should reveal which invariant you violated

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

### Adding New Features

For new features that need verification, follow this strict order:

```
Step 1: PROOFS FIRST - Specify in Lean
  └─→ Add types to Types.lean
  └─→ Add specifications to appropriate module
  └─→ Mark implementation-dependent properties as `axiom`
  └─→ Prove mathematical properties as `theorem`
  └─→ Run: cd lean && lake build

Step 2: PROPERTY TESTS - Before implementation
  └─→ Write proptest functions that encode the Lean invariants
  └─→ Tests will fail initially (no implementation yet)
  └─→ These tests define the contract your code must satisfy
  └─→ Run: cargo test proptest -- --test-threads=1 (expect failures)

Step 3: IMPLEMENTATION - Make tests pass
  └─→ Write code that satisfies property tests
  └─→ Use type-level wrappers for compile-time checks
  └─→ Add runtime contracts in contracts.rs
  └─→ Run: cargo test (all property tests should pass)

Step 4: FUZZ TESTS - Find edge cases
  └─→ Create fuzz targets for the new feature
  └─→ Run: cargo +nightly fuzz run <target> -- -max_total_time=60
  └─→ Fix any panics or invariant violations found
  └─→ Add regression tests for bugs found

Step 5: E2E TESTS - Verify user-facing behavior
  └─→ Write Playwright tests for the feature
  └─→ Run: bun run test:e2e -- --grep "<feature>"

Step 6: VERIFY - Full verification pass
  └─→ Run: cargo xtask verify
  └─→ All Lean proofs, property tests, and E2E tests must pass
```

**Why this order matters:**
- Proofs catch logical errors before you write any code
- Property tests ensure implementation matches specification
- Fuzz tests find edge cases humans miss
- E2E tests verify the feature works end-to-end

### Interpreting Verification Failures

| Failure | Meaning | Action |
|---------|---------|--------|
| `lake build` fails | Lean proof broken | Your change violated a mathematical property |
| Contract panic | Runtime invariant violated | Your code produces invalid data |
| Property test fails | Random input found a bug | Check edge cases in your implementation |
| Constant alignment fails | Rust/Lean drift | Update the lagging side to match |

### Lean Spec Quick Reference

```lean
-- Types.lean: Data structure definitions
structure SuffixEntry where doc_id : Nat; offset : Nat

-- SuffixArray.lean: Sorting and completeness
def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop

-- BinarySearch.lean: Search correctness
axiom findFirstGe_bounds : findFirstGe sa texts target ≤ sa.size

-- Scoring.lean: Ranking invariants
theorem title_beats_heading : baseScore .title - maxBoost > baseScore .heading + maxBoost

-- Levenshtein.lean: Edit distance properties
theorem fuzzyScore_monotone : d1 ≤ d2 → fuzzyScore d2 max ≤ fuzzyScore d1 max
```

### Red Flags That Require Lean Review

Stop and check Lean specs if you see yourself:

- Changing any numeric constant
- Modifying comparison logic (`<`, `<=`, `>`, `>=`)
- Changing array indexing or bounds
- Modifying sort comparators
- Adding new match arms to scoring
- Bypassing validation wrappers "just this once"

## WASM-to-Frontend Integration

Sieve is consumed by the frontend via WASM. **Verifying the Rust/WASM layer is not enough**—you must trace data flow through ALL frontend code paths.

### The Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SIEVE BOUNDARY                                  │
│  types.rs → binary.rs → wasm.rs → sieve.js (generated) → sieve.d.ts        │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           FRONTEND BOUNDARY                                  │
│  SearchIndexState.svelte.ts → SearchState.svelte.ts → SearchModal.svelte    │
│                                      ↓                                       │
│                           SearchResultItem.svelte                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Integration Checklist

When adding a new field to search results (like `section_id`):

```
□ 1. RUST: Add field to types.rs struct
□ 2. RUST: Populate field in inverted.rs/index.rs
□ 3. RUST: Encode field in binary.rs
□ 4. RUST: Expose field in wasm.rs SearchResultOutput
□ 5. WASM: Rebuild with wasm-pack build --target web --release
□ 6. TS TYPES: Update SearchResult interface in SearchState.svelte.ts
□ 7. TS HELPER: Create/update helper function (e.g., buildResultUrl)
□ 8. SVELTE: Update ALL components that consume search results
     - SearchResultItem.svelte (click handler)
     - SearchModal.svelte (keyboard Enter handler)  ← EASY TO MISS
     - Any other consumers
□ 9. E2E TESTS: Test ALL interaction methods (click AND keyboard AND touch)
□ 10. VERIFY: grep for raw field access, ensure helper is used everywhere
```

### Post-WASM Verification

After any WASM change that affects the JS interface:

```bash
# 1. Rebuild WASM
wasm-pack build --target web --release --features wasm

# 2. Test WASM directly in Node
node --input-type=module -e "
import initWasm, { SieveSearcher } from './pkg/sieve.js';
import { readFileSync } from 'fs';
const wasmBytes = readFileSync('./pkg/sieve_bg.wasm');
await initWasm(wasmBytes);
const indexBytes = readFileSync('/path/to/index.sift');
const searcher = new SieveSearcher(new Uint8Array(indexBytes));
console.log(JSON.stringify(searcher.search('test', 3), null, 2));
"

# 3. Run all tests
cargo test
```

### Common Integration Bugs

| Bug Pattern | How It Manifests | Prevention |
|-------------|------------------|------------|
| Helper not used everywhere | Click works, keyboard doesn't | grep for raw field access |
| Field missing from TS interface | TypeScript error or undefined | Check sieve.d.ts matches SearchResult |
| WASM not rebuilt | Old behavior persists | Always rebuild after wasm.rs changes |
| E2E tests incomplete | Works in dev, fails in production | Test click AND keyboard AND Enter |

## More Documentation

- **Binary format**: See `docs/architecture.md`
- **Verification details**: See `docs/verification.md`
