# Formal Verification Guide

This codebase uses formal verification to prevent silent data corruption. This guide explains the verification architecture, how to use it, and how to work safely within it.

## For AI Agents

<aside class="callout callout-warning">
<div class="callout-title">Critical</div>

Breaking invariants causes silent data corruption. Run `cargo xtask verify` after every change.

</aside>

**Mandatory workflow:**

```bash
cargo xtask verify    # Before AND after changes
cargo xtask check     # Quick check during development
```

**You cannot:** Change scoring constants without updating Lean proofs. Bypass type wrappers like `ValidatedSuffixEntry`. Remove INVARIANT comments. Silence contract checks.

**You can safely:** Add tests. Improve error messages. Add API functions using existing validated types. Make documentation changes.

**If tests fail:** Your code is wrong, not the test. Read the Lean spec to understand why.

---

## The Verification Architecture

Bug detection scales logarithmically with decomposition depth, not linearly with test count. We decompose verification into focused layers, each catching different bug classes.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              LEAN PROOFS                                    │
│                             (49 theorems)                                   │
│                                                                             │
│   Mathematical truth. If `lake build` succeeds, the math is correct.        │
│   Proves: scoring bounds, tier exclusion, binary search correctness.        │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ORACLE DIFFERENTIAL                                │
│                     (Ground truth comparison)                               │
│                                                                             │
│   ┌───────────────────┐              ┌───────────────────┐                  │
│   │      ORACLE       │              │       RUST        │                  │
│   │     (simple)      │    ══?══     │   (optimized)     │                  │
│   │                   │              │                   │                  │
│   │  O(n²) sort       │              │  SA-IS O(n)       │                  │
│   │  Wagner-Fischer   │              │  Bounded Lev      │                  │
│   │  Linear scan      │              │  Binary search    │                  │
│   └───────────────────┘              └───────────────────┘                  │
│                                                                             │
│   If they disagree, the oracle is right. 12 differential tests.             │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       PROPERTY-BASED TESTS                                  │
│                    (307 properties via proptest)                            │
│                                                                             │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│   │ suffix_     │  │ inverted_   │  │ section_    │  │ scoring_    │        │
│   │ array_props │  │ index_props │  │ props       │  │ props       │        │
│   │             │  │             │  │             │  │             │        │
│   │ • Sorted    │  │ • Posting   │  │ • Non-      │  │ • Field     │        │
│   │ • Complete  │  │   order     │  │   overlap   │  │   hierarchy │        │
│   │ • LCP ok    │  │ • Doc freq  │  │ • Valid IDs │  │ • Monotone  │        │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                                             │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│   │ binary_     │  │ tier_       │  │ search_     │  │ fuzzy_      │        │
│   │ props       │  │ integration │  │ results     │  │ dfa         │        │
│   │             │  │             │  │             │  │             │        │
│   │ • Varint RT │  │ • T1⊂Full   │  │ • Sorted    │  │ • Accepts   │        │
│   │ • SA encode │  │ • T2∩T1=∅   │  │ • No dupes  │  │   within d  │        │
│   │ • Postings  │  │ • Union=All │  │ • Valid IDs │  │ • Rejects   │        │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            FUZZ TESTING                                     │
│                          (11 fuzz targets)                                  │
│                                                                             │
│   cargo +nightly fuzz run <target>                                          │
│                                                                             │
│   • search_queries      • tier_merging        • varint_decode               │
│   • section_bounds      • score_calculation   • levenshtein_matching        │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          MUTATION TESTING                                   │
│                       (cargo-mutants, 60% threshold)                        │
│                                                                             │
│   Systematically corrupts code to verify tests catch bugs:                  │
│                                                                             │
│   Original:   if len > max { return false }                                 │
│   Mutant #1:  if len >= max { return false }   ← Tests must catch this      │
│   Mutant #2:  if len < max { return false }    ← Tests must catch this      │
│                                                                             │
│   If tests pass on a mutant, we have a test gap.                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             KANI PROOFS                                     │
│                       (Bounded model checking)                              │
│                                                                             │
│   Proves absence of panics for ALL possible inputs:                         │
│                                                                             │
│   #[kani::proof]                                                            │
│   fn varint_decode_never_panics() {                                         │
│       let bytes: [u8; 11] = kani::any();                                    │
│       let _ = decode_varint(&bytes);  // Proven: never panics               │
│   }                                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### What Each Layer Catches

| Layer              | Catches                      | Example Bug               |
|--------------------|------------------------------|---------------------------|
| Lean Proofs        | Mathematical impossibilities | Score ordering wrong      |
| Oracle Differential| Optimization bugs            | Byte vs char length       |
| Property Tests     | Invariant violations         | Suffix array unsorted     |
| Fuzz Testing       | Crashes and edge cases       | Varint overflow           |
| Mutation Testing   | Weak test coverage           | Missing assertion         |
| Kani Proofs        | Panic conditions             | Array out of bounds       |

---

## How Oracle Differential Testing Works

Oracle testing compares optimized implementations against simple, obviously-correct reference implementations. The oracle is always right.

```
    proptest generates random input
                    │
                    ▼
          ┌─────────────────┐
          │  query: "café"  │
          │  max_dist: 2    │
          │  target: "cafe" │
          └────────┬────────┘
                   │
         ┌─────────┴─────────┐
         ▼                   ▼
   ┌───────────┐       ┌───────────┐
   │  ORACLE   │       │   RUST    │
   │           │       │           │
   │  Wagner-  │       │  Bounded  │
   │  Fischer  │       │  Lev with │
   │   O(nm)   │       │  early    │
   │           │       │  exit     │
   └─────┬─────┘       └─────┬─────┘
         │                   │
         ▼                   ▼
     dist = 1          within = true
         │                   │
         └─────────┬─────────┘
                   ▼
         ┌─────────────────┐
         │    COMPARE      │
         │                 │
         │  expected:      │
         │  dist <= max    │
         │  1 <= 2 = true  │
         │                 │
         │  rust: true     │
         │                 │
         │    ✓ MATCH      │
         └─────────────────┘
```

**Available oracles:**

| Oracle                       | Complexity   | Compared Against           |
|------------------------------|--------------|----------------------------|
| `oracle_suffix_array`        | O(n² log n)  | SA-IS algorithm O(n)       |
| `oracle_lower_bound`         | O(n)         | Binary search O(log n)     |
| `oracle_levenshtein`         | O(nm)        | Bounded Levenshtein        |
| `oracle_common_prefix_len`   | O(n)         | Optimized prefix matching  |
| `oracle_encode/decode_varint`| O(1)         | Optimized LEB128           |

**Location:** `tests/property/oracles.rs` (implementations), `tests/property/oracle_differential.rs` (tests)

---

## Key Invariants

These are the properties that must never be violated.

### Suffix Entry Well-Formedness

Every suffix entry must point to a valid location within the document corpus.

```lean
def SuffixEntry.WellFormed (e : SuffixEntry) (texts : Array String) : Prop :=
  e.doc_id < texts.size ∧ e.offset ≤ texts[e.doc_id].length
```

**Enforcement:** `ValidatedSuffixEntry` wrapper (compile-time), `check_suffix_entry_valid` (runtime debug assertion).

### Suffix Array Sortedness

The suffix array must be lexicographically sorted. Binary search correctness depends on this.

```lean
def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
  ∀ i j, i < j → suffixAt texts sa[i] ≤ suffixAt texts sa[j]
```

**Enforcement:** `SortedSuffixArray` wrapper, `check_suffix_array_sorted` assertion, `is_suffix_array_sorted` predicate.

### Field Type Dominance

Title matches must always outrank heading matches, which must always outrank content matches, regardless of position bonuses.

```lean
theorem title_beats_heading :
    baseScore .title - maxPositionBoost > baseScore .heading + maxPositionBoost

theorem heading_beats_content :
    baseScore .heading - maxPositionBoost > baseScore .content + maxPositionBoost
```

**Constants:** Title=100, Heading=10, Content=1, MaxBoost=0.5. The gap between adjacent levels must exceed `2 × MaxBoost`.

### LCP Array Correctness

The longest common prefix array must accurately reflect the common prefix length between adjacent suffix array entries.

```lean
def LcpCorrect (sa lcp texts) : Prop :=
  lcp.size = sa.size ∧
  lcp[0] = 0 ∧
  ∀ i > 0, lcp[i] = commonPrefix(sa[i-1], sa[i]).length
```

### Index Well-Formedness

All index components must be consistent: document count matches text count, LCP length matches suffix array length, all entries are valid, and the suffix array is sorted.

```lean
def SearchIndex.WellFormed (idx : SearchIndex) : Prop :=
  idx.docs.size = idx.texts.size ∧
  idx.lcp.size = idx.suffix_array.size ∧
  Sorted idx.suffix_array idx.texts ∧
  (∀ e ∈ idx.suffix_array, SuffixEntry.WellFormed e idx.texts)
```

**Enforcement:** `WellFormedIndex` wrapper, `check_index_well_formed` assertion.

---

## Running Verification

### Quick Commands

```bash
cargo xtask verify    # Full 11-step verification (before commits)
cargo xtask check     # Quick check: tests + clippy (during development)
cargo xtask test      # Just tests
cargo xtask lean      # Just Lean proofs
cargo xtask kani      # Kani model checking (~5 minutes)
```

### The 11-Step Verification Pipeline

`cargo xtask verify` runs these steps in order:

1. **Lean proofs** - Mathematical specifications compile
2. **Constants** - Rust/Lean constant alignment
3. **Spec drift** - Lean/Rust specification alignment
4. **Invariants** - INVARIANT markers present in source
5. **Clippy** - Lint checks pass
6. **Release build** - Binary compiles in release mode
7. **Test fixtures** - E2E index building succeeds
8. **Rust tests** - All 307 property tests pass
9. **WASM parity** - Native and WASM produce identical results
10. **Browser E2E** - Playwright tests pass
11. **Mutations** - 60%+ mutation detection rate

### Running Specific Test Suites

```bash
# Oracle differential tests
cargo test oracle_differential --test property

# All property tests
cargo test --test property -- --test-threads=1

# Fuzz testing (requires nightly)
cargo +nightly fuzz run search_queries -- -max_total_time=60

# Mutation testing
cargo mutants --package sorex -- --lib
```

### Kani Model Checking

Kani proves panic-freedom for all possible inputs, not just random samples. Run separately due to long runtime:

```bash
cargo xtask kani    # ~5 minutes
```

**Proofs in `kani-proofs/`:**

| Proof                            | Guarantee                                    |
|----------------------------------|----------------------------------------------|
| `verify_encode_varint_no_panic`  | Encoding any u64 never panics                |
| `verify_decode_varint_no_panic`  | Decoding any byte sequence never panics      |
| `verify_varint_roundtrip`        | `decode(encode(x)) == x` for all x           |
| `verify_decode_empty_input`      | Empty bytes return error, not crash          |
| `verify_decode_rejects_overlong` | Malformed 11+ byte varints rejected          |

---

## Refactoring Guidelines

### Adding a New Field Type

1. Add variant to `FieldType` enum in `types.rs`
2. Add base score in `scoring.rs::field_type_score`
3. Update Lean spec in `Scoring.lean`
4. Add dominance proof if it ranks between existing types
5. Update `check_field_hierarchy` in `contracts.rs`
6. Add test case for the new ranking

### Modifying Binary Search

1. Binary search correctness depends on the `Sorted` invariant
2. Preserve: "all before result are less than target"
3. Preserve: "result and after are greater than or equal to target"
4. Run: `cargo test lean_proptest`

### Modifying Index Construction

1. Preserve well-formedness invariants
2. Verify with `check_index_complete(&index)` in debug builds
3. Run full test suite and Lean proofs

### Adding New Scoring Factors

1. Document in `Scoring.lean`
2. Maximum impact must be less than the gap between field types
3. Add property test for the new factor

---

## Verification Status

| Component           | Lean Status   | Rust Status                 |
|---------------------|---------------|-----------------------------|
| Type definitions    | ✓ Specified   | ✓ Implemented               |
| Suffix sortedness   | Axiom         | Property tested + oracle    |
| Suffix completeness | Axiom         | Property tested             |
| LCP correctness     | Axiom         | Unit tested                 |
| Field dominance     | ✓ Proven      | Statically checked          |
| Binary search       | Axiom         | Property tested + oracle    |
| Edit distance       | Axiom         | Unit tested + oracle        |
| Varint encoding     | ✓ Kani proven | Panic-free for all inputs   |
| Varint decoding     | ✓ Kani proven | Panic-free for all inputs   |

**Summary:**
- 49 Lean theorems + 5 Kani proofs
- 18 axioms (empirically verified via property tests)
- 307 property tests across 17 focused modules
- 12 oracle differential tests
- 11 fuzz targets
- 60%+ mutation detection (CI enforced)

---

## Limits of Formal Verification

Formal verification provides strong guarantees but has inherent limitations.

### What We Prove vs Axiomatize

| Property                     | Status      | Rationale                                      |
|------------------------------|-------------|------------------------------------------------|
| Field ranking dominance      | **Proven**  | Critical for correctness; algebraically provable|
| Levenshtein triangle inequality | **Proven** | Mathematical property                          |
| Fuzzy score monotonicity     | **Proven**  | Algebraic                                      |
| Suffix array sortedness      | Axiom       | Would require verifying Rust's sort            |
| Binary search correctness    | Axiom       | Would require full algorithm verification      |

**Why axioms?** Proving Rust's `sort()` is correct would require formalizing the entire standard library. Instead, we test sortedness as a post-condition and compare against oracles.

### What Formal Verification Cannot Guarantee

1. **Performance** - Proofs say nothing about latency or throughput
2. **Correct specification** - If the Lean spec is wrong, the implementation will be "correctly wrong"
3. **External dependencies** - We don't verify wasm-bindgen, serde, etc.
4. **Concurrency** - Proofs assume single-threaded execution
5. **Floating point** - Score calculations use f64 without formal verification

### Pragmatic Choices

**Stop words:** Filtered at index time. Linguistic judgment, not mathematical property.

**Edit distance limit:** Max distance 2. Empirical choice balancing relevance and performance.

**Scoring constants:** 100/10/1. Chosen for debuggability; proof only requires dominance property.

### When to Verify vs Test

| Situation                            | Approach                    |
|--------------------------------------|-----------------------------|
| Mathematical invariant               | Prove in Lean               |
| Data structure well-formedness       | Type-level wrapper          |
| Algorithm correctness                | Axiom + property test       |
| Edge cases and boundaries            | Unit tests                  |
| Real-world scenarios                 | Integration tests           |
| Performance                          | Benchmarks                  |

---

## Quick Reference

### Before Refactoring

```bash
cargo xtask verify    # Establish baseline
```

### After Refactoring

```bash
cargo xtask check     # Quick validation
cargo xtask verify    # Full verification before commit
```

### If Tests Fail

1. Identify which invariant is violated
2. Read the corresponding Lean spec
3. Fix your code to preserve the invariant
4. Add regression test

---

## Related Documentation

- [Architecture](./architecture.md) - System overview
- [Rust API](./rust.md) - Type-level invariants
- [Benchmarks](./benchmarks.md) - Performance testing
- [Verification Issues](./verification-issues.md) - Known gaps
