# Formal Verification Guide

This document describes the formal verification infrastructure for the search crate and provides guidelines for safe refactoring.

## For AI Agents (Claude, Copilot, etc.)

**READ THIS SECTION FIRST**

This codebase has formal verification. You MUST follow these rules:

### Mandatory Workflow

```bash
# BEFORE making any change:
cargo xtask verify            # Full verification baseline

# AFTER every change:
cargo xtask check             # Quick check (tests + clippy)
cargo xtask verify            # Full verification before commit
```

### What You CANNOT Do

1. **Change scoring constants** in `scoring.rs` without updating Lean proofs
2. **Bypass type wrappers** like `ValidatedSuffixEntry` - they exist to prevent bugs
3. **Remove INVARIANT comments** - they mark safety-critical code
4. **Silence contract checks** in `contracts.rs` - they catch YOUR bugs
5. **Modify the sort comparator** in `index.rs` - binary search depends on it

### What You CAN Do Safely

1. Add new tests (always safe)
2. Improve error messages
3. Add new public API functions that use existing validated types
4. Documentation changes
5. Performance optimizations that don't change semantics

### If Tests Fail

1. **Your code is wrong**, not the test
2. Read the Lean specification to understand WHY
3. Revert your change and try a different approach
4. Never weaken or skip tests

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         LEAN SPECIFICATIONS                          │
│  lean/SearchVerified/*.lean                                          │
│  - Types.lean: Type definitions and well-formedness predicates       │
│  - SuffixArray.lean: Sorted, Complete, LcpCorrect properties        │
│  - BinarySearch.lean: Search correctness theorems                   │
│  - Scoring.lean: Field hierarchy dominance proofs                   │
│  - Levenshtein.lean: Edit distance bounds and properties            │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      RUST IMPLEMENTATION                             │
│  src/                                                                │
│  ├── types.rs      ←──→ Types.lean                                  │
│  ├── index.rs      ←──→ SuffixArray.lean                            │
│  ├── search.rs     ←──→ BinarySearch.lean                           │
│  ├── scoring.rs    ←──→ Scoring.lean                                │
│  ├── levenshtein.rs ←──→ Levenshtein.lean                           │
│  ├── verified.rs   ←──→ Type-level invariant wrappers               │
│  └── contracts.rs  ←──→ Runtime debug assertions                    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       VERIFICATION LAYERS                            │
│  1. Lean Proofs: Formal mathematical proofs (5 proven, 18 axioms)   │
│  2. Type-Level: Compile-time guarantees via wrapper types           │
│  3. Contracts: Runtime debug_assert! checks                         │
│  4. Property Tests: Randomized testing via proptest                 │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Invariants

### 1. Suffix Entry Well-Formedness

**Lean Spec** (`Types.lean`):
```lean
def SuffixEntry.WellFormed (e : SuffixEntry) (texts : Array String) : Prop :=
  e.doc_id < texts.size ∧ e.offset ≤ texts[e.doc_id].length
```

**Rust Enforcement**:
- `ValidatedSuffixEntry` (verified.rs) - compile-time check at construction
- `check_suffix_entry_valid` (contracts.rs) - runtime debug assertion

**Safe Refactoring**:
- Never create `SuffixEntry` with unchecked `doc_id` or `offset`
- Use `ValidatedSuffixEntry::new()` for guaranteed valid entries
- When modifying `build_index`, ensure all entries are created from valid `(doc_id, offset)` pairs

### 2. Suffix Array Sortedness

**Lean Spec** (`SuffixArray.lean`):
```lean
def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
  ∀ i j : Nat, i < sa.size → j < sa.size → i < j →
    suffixAt texts sa[i] ≤ suffixAt texts sa[j]
```

**Rust Enforcement**:
- `SortedSuffixArray` (verified.rs) - validates sortedness at construction
- `check_suffix_array_sorted` (contracts.rs) - runtime assertion
- `is_suffix_array_sorted` (index.rs) - predicate function

**Safe Refactoring**:
- Binary search correctness **depends** on this invariant
- After any modification to `build_index`, verify with `is_suffix_array_sorted`
- Never insert into suffix array without re-sorting
- The sort comparator must use `suffix_at` for lexicographic ordering

### 3. Field Type Dominance

**Lean Spec** (`Scoring.lean`):
```lean
theorem title_beats_heading :
    baseScore .title - maxPositionBoost > baseScore .heading + maxPositionBoost

theorem heading_beats_content :
    baseScore .heading - maxPositionBoost > baseScore .content + maxPositionBoost
```

**Rust Enforcement**:
- `check_field_hierarchy` (contracts.rs) - static assertion
- Property test `lean_proptest_field_hierarchy_preserved`

**Safe Refactoring**:
- Scoring constants: Title=100, Heading=10, Content=1, MaxBoost=0.5
- If changing any constant, verify: `Title - MaxBoost > Heading + MaxBoost`
- If changing any constant, verify: `Heading - MaxBoost > Content + MaxBoost`
- The gap must be at least `2 * MaxBoost` between adjacent levels

### 4. LCP Array Correctness

**Lean Spec** (`SuffixArray.lean`):
```lean
def LcpCorrect (sa : Array SuffixEntry) (lcp : Array Nat) (texts : Array String) : Prop :=
  lcp.size = sa.size ∧
  (sa.size > 0 → lcp[0] = 0) ∧
  ∀ i, i > 0 → lcp[i] = commonPrefix(sa[i-1], sa[i]).length
```

**Rust Enforcement**:
- `check_lcp_correct` (contracts.rs) - runtime assertion
- Test `lean_spec_lcp_correct`

**Safe Refactoring**:
- LCP must be rebuilt whenever suffix array changes
- `lcp[0]` must always be 0
- `lcp[i]` = common prefix length of `suffix_array[i-1]` and `suffix_array[i]`

### 5. Index Well-Formedness

**Lean Spec** (`Types.lean`):
```lean
def SearchIndex.WellFormed (idx : SearchIndex) : Prop :=
  idx.docs.size = idx.texts.size ∧
  idx.lcp.size = idx.suffix_array.size ∧
  Sorted idx.suffix_array idx.texts ∧
  (∀ e ∈ idx.suffix_array, SuffixEntry.WellFormed e idx.texts)
```

**Rust Enforcement**:
- `WellFormedIndex` (verified.rs) - validates all invariants
- `check_index_well_formed` (contracts.rs) - runtime assertion

**Safe Refactoring**:
- `docs.len() == texts.len()` must hold
- `lcp.len() == suffix_array.len()` must hold
- All suffix entries must be valid for `texts`
- Suffix array must be sorted

## Refactoring Guidelines

### Adding a New Field Type

1. Add variant to `FieldType` enum in `types.rs`
2. Add base score in `scoring.rs::field_type_score`
3. Update Lean spec in `Scoring.lean`
4. Add dominance proof if it should rank between existing types
5. Update `check_field_hierarchy` in `contracts.rs`
6. Add test case for the new field type ranking

### Modifying Binary Search

1. Binary search correctness depends on `Sorted` invariant
2. Any change must preserve: "all before result are < target"
3. Any change must preserve: "result and after are >= target"
4. Use `check_binary_search_result` to verify bounds
5. Run property tests: `cargo test lean_proptest`

### Modifying Index Construction

1. Preserve well-formedness invariants
2. After changes, verify:
   ```rust
   use search::contracts::check_index_complete;
   check_index_complete(&index);  // In debug builds
   ```
3. Run full test suite: `cargo test`
4. Verify Lean builds: `cd lean && lake build`

### Adding New Scoring Factors

1. Document the factor in `Scoring.lean`
2. Ensure it doesn't violate field type dominance
3. The maximum impact must be less than the gap between field types
4. Add property test for the new factor

## Running Verification

### Using cargo xtask (Recommended)

```bash
cargo xtask verify    # Full verification suite
cargo xtask check     # Quick check (no Lean)
cargo xtask test      # Just tests
cargo xtask lean      # Just Lean proofs
cargo xtask bench     # Benchmarks
```

### Manual Commands

```bash
# Lean proofs
cd lean && lake build && cd ..

# All tests
cargo test

# Property tests only
cargo test proptest

# Integration tests
cargo test --test invariants
cargo test --test property
cargo test --test integration

# Clippy
cargo clippy -- -D warnings
```

### Debug Contracts
Contracts are enabled in debug builds:
```bash
cargo build       # Contracts enabled
cargo build -r    # Contracts disabled (release)
```

## Verification Status

| Component | Lean Status | Rust Status |
|-----------|-------------|-------------|
| Type definitions | ✓ Specified | ✓ Implemented |
| Suffix sortedness | Axiom | Property tested |
| Suffix completeness | Axiom | Property tested |
| LCP correctness | Axiom | Unit tested |
| Field dominance | ✓ Proven | Statically checked |
| Binary search bounds | Axiom | Property tested |
| Edit distance bounds | Axiom | Unit tested |

**Proven Theorems**: 5
**Axioms (empirically verified)**: 18
**Property Tests**: 10+
**Type-Level Invariants**: 3 (ValidatedSuffixEntry, SortedSuffixArray, WellFormedIndex)

## Quick Reference

### Before Any Refactoring
```bash
# Verify current state
cargo test
cd lean && lake build
```

### After Refactoring
```bash
# Run all tests
cargo test

# Check contracts in debug mode
cargo build && cargo test

# Verify Lean specs still match
cd lean && lake build

# Run property tests extensively
cargo test proptest -- --test-threads=1
```

### If Tests Fail
1. Check which invariant is violated
2. Consult the corresponding Lean spec
3. Ensure your change preserves the invariant
4. Add regression test for the case

---

## Related Documentation

- [Architecture](./architecture.md) — Binary format, system overview
- [Algorithms](./algorithms.md) — Suffix arrays, Levenshtein automata
- [Benchmarks](./benchmarks.md) — Performance comparisons with other libraries
- [Integration](./integration.md) — WASM setup, browser integration
- [Contributing](./contributing.md) — How to contribute safely
