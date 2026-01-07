# Contributing to Sieve

Sieve has formal verification. This is both a feature and a constraint - it means certain bugs are impossible, but it also means you can't just change things without updating proofs.

This guide explains how to contribute safely.

---

## Before You Start

```bash
# Clone and verify the baseline
git clone https://github.com/harryzorus/sieve.git
cd sieve
cargo xtask verify
```

If verification fails on a clean clone, stop. Something is wrong with the environment, not your changes.

### Required Tools

- **Rust** (stable)  -  `rustup install stable`
- **Lean 4**  -  `elan install leanprover/lean4:v4.3.0`
- **wasm-pack** (optional)  -  `cargo install wasm-pack`

---

## The Golden Rule

> **Never silence a failing check. The check is right; your code is wrong.**

Verification failures aren't bugs in the verification - they're bugs in your code. If a property test fails, if a Lean proof breaks, if a contract panics, your change violated an invariant.

---

## What You Can Safely Change

### Low Risk (No Proofs Affected)

- Documentation and comments
- Error messages and logging
- Adding new tests
- Performance optimizations that don't change behavior
- Adding optional features behind feature flags

### Medium Risk (May Require Proof Updates)

- Adding fields to existing types
- Adding new search options
- Changing serialization format
- Modifying scoring within proven bounds

### High Risk (Definitely Requires Proof Updates)

- Changing type definitions in `types.rs`
- Modifying scoring constants in `scoring.rs`
- Changing binary search logic in `search.rs`
- Altering suffix array construction in `index.rs`
- Modifying edit distance computation in `levenshtein.rs`

---

## Development Workflow

### 1. Understand What You're Changing

Before modifying any verified code, read the corresponding Lean specification:

| Rust File | Lean Specification |
|-----------|-------------------|
| `types.rs` | `lean/SearchVerified/Types.lean` |
| `index.rs` | `lean/SearchVerified/SuffixArray.lean` |
| `search.rs` | `lean/SearchVerified/BinarySearch.lean` |
| `scoring.rs` | `lean/SearchVerified/Scoring.lean` |
| `levenshtein.rs` | `lean/SearchVerified/Levenshtein.lean` |
| `inverted.rs` | `lean/SearchVerified/InvertedIndex.lean` |

### 2. Make Your Changes

Write code as normal, but be aware of invariants documented in [Verification](./verification.md).

### 3. Run Verification

```bash
cargo xtask verify
```

This runs:
1. All Rust tests (unit, integration, property)
2. All Lean proofs
3. Clippy lints
4. Constant alignment checks (Rust ↔ Lean)

### 4. Fix Failures

If verification fails:

**Rust test failure**: Your code has a bug. Fix it.

**Property test failure**: Your code violated an invariant on a random input. The test will print the failing input - use it to debug.

**Lean proof failure**: You changed something the proofs depend on. Either:
- Revert your change, or
- Update the Lean specifications to match (see below)

**Constant alignment failure**: Rust and Lean constants drifted. Update whichever is behind.

---

## Updating Lean Proofs

### When to Update Proofs

Update proofs when:
- Adding new types that need verification
- Changing existing type structures
- Modifying constants (scoring weights, thresholds)
- Adding new invariants

Don't update proofs when:
- Adding optional fields
- Changing internal implementation details
- Optimizing code without changing semantics

### How to Update Proofs

1. **Understand the current spec**

```bash
cat lean/SearchVerified/Types.lean
```

2. **Modify the Lean file to match your Rust changes**

```lean
-- If you added a field to SearchDoc in Rust:
structure SearchDoc where
  id : Nat
  title : String
  excerpt : String
  href : String
  kind : String
  newField : String  -- Add here too
```

3. **Rebuild Lean**

```bash
cd lean && lake build
```

4. **Fix any broken proofs**

If adding a field breaks proofs, it's because some property assumed the old structure. Think about whether the property still holds with your change.

### Adding New Theorems

If you're adding a new invariant that should be verified:

1. **Write the property in Lean**

```lean
-- In lean/SearchVerified/YourModule.lean
theorem your_property :
  ∀ x, someCondition x → otherCondition x := by
  intro x hcond
  -- proof goes here
```

2. **Add a corresponding property test in Rust**

```rust
// In tests/property.rs
proptest! {
    #[test]
    fn prop_your_property(input in any_strategy()) {
        // Test the same property
        prop_assert!(your_property(&input));
    }
}
```

3. **Add runtime contract (optional)**

```rust
// In src/contracts.rs
pub fn check_your_property(input: &T) {
    debug_assert!(
        your_property(input),
        "INVARIANT VIOLATED: your_property"
    );
}
```

---

## Testing

### Run All Tests

```bash
cargo test
```

### Run Property Tests with More Cases

```bash
PROPTEST_CASES=1000 cargo test proptest
```

### Run Fuzzing (Requires Nightly)

```bash
rustup install nightly
cargo +nightly fuzz run section_boundaries -- -max_total_time=60
```

### Run Benchmarks

```bash
cargo xtask bench
```

---

## Code Style

### Invariant Documentation

Every function that maintains an invariant should document it:

```rust
/// Build suffix array from vocabulary.
///
/// **INVARIANT**: Output is sorted lexicographically by suffix.
/// **Lean spec**: `SuffixArray.lean`, `Sorted` definition.
pub fn build_vocab_suffix_array(vocab: &[String]) -> Vec<VocabSuffixEntry> {
    // ...
}
```

### Type-Level Wrappers

Use newtypes to enforce invariants at compile time:

```rust
// Bad: raw type, can be invalid
let entry = SuffixEntry { doc_id: 999, offset: 0 };

// Good: validated wrapper, can't be invalid
let entry = ValidatedSuffixEntry::new(
    SuffixEntry { doc_id: 0, offset: 5 },
    &texts
)?;
```

### Runtime Contracts

Add debug assertions for invariants that can't be checked at compile time:

```rust
pub fn search(index: &SearchIndex, query: &str) -> Vec<SearchResult> {
    // Contract: index must be well-formed
    debug_assert!(check_index_well_formed(index));

    // ... search logic ...

    // Contract: results are sorted by score descending
    debug_assert!(results.windows(2).all(|w| w[0].score >= w[1].score));

    results
}
```

---

## Pull Request Checklist

Before submitting:

- [ ] `cargo xtask verify` passes
- [ ] New code has appropriate tests
- [ ] Documentation updated if behavior changed
- [ ] Lean specs updated if types/constants changed
- [ ] No new clippy warnings
- [ ] Commit messages are descriptive

---

## Common Mistakes

### Bypassing Validation

```rust
// Wrong: bypasses well-formedness check
let entry = SuffixEntry { doc_id: 5, offset: 0 };

// Right: uses validated wrapper
let entry = ValidatedSuffixEntry::new(SuffixEntry { doc_id: 0, offset: 2 }, &texts)?;
```

### Changing Constants Without Proof Update

```rust
// Wrong: breaks field_type_dominance theorem
FieldType::Title => 50.0,  // Changed from 100.0

// Right: update Lean first, then Rust
// 1. Edit Scoring.lean: def baseScore | .title => 50
// 2. Run: cd lean && lake build
// 3. If proofs fail, fix them or reconsider the change
// 4. Then update Rust
```

### Silencing Contract Violations

```rust
// Wrong: hiding bugs
#[allow(debug_assertions)]
fn buggy_function() { ... }

// Right: fix the bug
fn fixed_function() { ... }
```

### Modifying Sorted Data After Creation

```rust
// Wrong: breaks sortedness invariant
let mut sa = build_suffix_array(&texts);
sa.push(new_entry);  // Not sorted anymore!

// Right: rebuild the entire index
let index = build_index(docs, texts);  // Maintains invariants
```

---

## Getting Help

- **Bug reports**: Open an issue with a minimal reproduction
- **Feature requests**: Open an issue describing the use case
- **Questions**: Open a discussion thread

When reporting verification failures, include:
1. The exact command that failed
2. The full error output
3. Your Rust and Lean versions

---

## Related Documentation

- [Architecture](./architecture.md): Binary format, system overview
- [Algorithms](./algorithms.md): Suffix arrays, Levenshtein automata
- [Benchmarks](./benchmarks.md): Performance comparisons with other libraries
- [Integration](./integration.md): WASM setup, browser integration
- [Verification](./verification.md): Formal verification guide

---

## License

By contributing, you agree that your contributions will be licensed under Apache-2.0.
