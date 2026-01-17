// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Procedural macros that generate Lean 4 specs from Rust code.
//!
//! The promise: write your types once in Rust, get Lean structures for free.
//! Annotate with `#[derive(LeanSpec)]` and the macro emits a corresponding
//! Lean 4 definition. Add `#[lean_verify]` to a function and you get theorem
//! statements with `sorry` placeholders. The proofs are your job.
//!
//! The real trick is `#[lean_proptest_verify]`. It generates both Lean specs
//! AND executable property tests from the same annotations. If the Lean proof
//! compiles and the proptests pass, you have reasonable confidence the Rust
//! matches its formal spec. Not certainty (that would require full verification)
//! but a good night's sleep.
//!
//! # How it works
//!
//! 1. **Spec generation**: `LeanSpec` and `lean_verify` emit Lean 4 code
//! 2. **Test generation**: `LeanProptest` and `lean_proptest` emit proptest harnesses
//! 3. **Runtime checks**: Generated tests verify the implementation matches the spec
//!
//! # Example
//!
//! ```ignore
//! use sorex_lean_macros::{LeanSpec, LeanProptest, lean_verify, lean_proptest};
//!
//! // Generate both Lean specs and proptest strategies
//! #[derive(LeanSpec, LeanProptest)]
//! #[lean(name = "SuffixEntry")]
//! pub struct SuffixEntry {
//!     #[lean(bounds = "0..1000")]
//!     pub doc_id: usize,
//!     #[lean(bounds = "0..10000")]
//!     pub offset: usize,
//! }
//!
//! // Generate Lean theorem and proptest property
//! #[lean_verify(
//!     spec = "suffix_array_sorted",
//!     ensures = "forall i j, i < j -> suffix[i] <= suffix[j]"
//! )]
//! fn build_index(docs: Vec<Doc>) -> SearchIndex { ... }
//!
//! // Property test derived from Lean specification
//! #[lean_proptest(spec = "search_finds_all_matches", cases = 1000)]
//! fn test_search_completeness(index in any::<SearchIndex>(), query in "[a-z]+") {
//!     // All matching suffixes are returned
//! }
//! ```

use proc_macro::TokenStream;

mod codegen;
mod lean_spec;
mod lean_verify;
mod proptest;
mod proptest_gen;

/// Derive macro for generating Lean type specifications from Rust structs.
///
/// # Attributes
///
/// - `#[lean(name = "LeanName")]` - Override the Lean type name
/// - `#[lean(invariant = "predicate")]` - Add a well-formedness predicate
///
/// # Generated Output
///
/// For each struct, generates:
/// - A Lean `structure` definition with translated field types
/// - A `WellFormed` predicate if invariants are specified
/// - A `LEAN_SPEC` constant containing the generated Lean code
#[proc_macro_derive(LeanSpec, attributes(lean))]
pub fn derive_lean_spec(input: TokenStream) -> TokenStream {
    lean_spec::derive(input)
}

/// Attribute macro for marking functions to generate Lean specifications.
///
/// # Attributes
///
/// - `spec = "name"` - Name for the generated specification
/// - `requires = "predicate"` - Precondition (Lean `Prop`)
/// - `ensures = "predicate"` - Postcondition (Lean `Prop`)
/// - `properties = ["prop1", "prop2"]` - Additional properties to prove
///
/// # Generated Output
///
/// - Function signature in Lean
/// - Theorem statements for pre/post conditions
/// - `sorry` placeholders for proofs
#[proc_macro_attribute]
pub fn lean_verify(attr: TokenStream, item: TokenStream) -> TokenStream {
    lean_verify::process(attr, item)
}

/// Attribute macro for inline property annotations.
///
/// These are collected during `lean_verify` processing and generate
/// additional theorem statements in the Lean output.
///
/// # Example
///
/// ```ignore
/// #[lean_property(
///     name = "field_type_dominates_position",
///     statement = "title_score - 0.5 > heading_score + 0.5"
/// )]
/// let base_score = field_type_score(&field_type);
/// ```
#[proc_macro_attribute]
pub fn lean_property(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Properties are collected during lean_verify processing
    // This macro just passes through the item unchanged
    let _ = attr; // Suppress unused warning
    item
}

// ============================================================================
// PROPERTY TESTING MACROS
// ============================================================================

/// Derive macro for generating proptest strategies from Rust structs.
///
/// This macro generates `Arbitrary` implementations that respect the bounds
/// and patterns specified in `#[lean(...)]` attributes, enabling property-based
/// testing that aligns with formal Lean specifications.
///
/// # Field Attributes
///
/// - `#[lean(bounds = "0..100")]` - Numeric range for usize/isize fields
/// - `#[lean(pattern = "[a-z]+")]` - Regex pattern for String fields
/// - `#[lean(size = "0..10")]` - Size range for Vec fields
/// - `#[lean(strategy = "...")]` - Custom proptest strategy expression
/// - `#[lean(fixed = "value")]` - Fixed value (not randomized)
///
/// # Generated Output
///
/// For each struct, generates an `impl Arbitrary` that:
/// - Uses the specified bounds/patterns for constrained generation
/// - Falls back to sensible defaults for unspecified fields
/// - Composes field strategies into a struct strategy
///
/// # Example
///
/// ```ignore
/// #[derive(LeanProptest)]
/// struct SuffixEntry {
///     #[lean(bounds = "0usize..1000")]
///     doc_id: usize,
///     #[lean(bounds = "0usize..10000")]
///     offset: usize,
/// }
///
/// // Generates:
/// impl Arbitrary for SuffixEntry {
///     type Strategy = BoxedStrategy<Self>;
///     fn arbitrary_with(_: ()) -> Self::Strategy {
///         (0usize..1000, 0usize..10000)
///             .prop_map(|(doc_id, offset)| SuffixEntry { doc_id, offset })
///             .boxed()
///     }
/// }
/// ```
#[proc_macro_derive(LeanProptest, attributes(lean))]
pub fn derive_lean_proptest(input: TokenStream) -> TokenStream {
    proptest::derive(input)
}

/// Attribute macro for generating property tests from Lean specifications.
///
/// This macro wraps a test function in a proptest! invocation, using the
/// property description from Lean annotations as documentation.
///
/// # Attributes
///
/// - `spec = "name"` - Name for the generated test module
/// - `property = "description"` - Property being tested (becomes doc comment)
/// - `cases = 1000` - Number of test cases (default: 256)
/// - `regression = true` - Generate regression test file
///
/// # Example
///
/// ```ignore
/// #[lean_proptest(
///     spec = "binary_search_finds_all",
///     property = "All suffixes starting with query are returned",
///     cases = 1000
/// )]
/// fn test_search(index: SearchIndex, query: String) {
///     let results = search(&index, &query);
///     for entry in &index.suffix_array {
///         let suffix = suffix_at(&index.texts, entry);
///         if suffix.starts_with(&query) {
///             assert!(results.contains(entry));
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn lean_proptest(attr: TokenStream, item: TokenStream) -> TokenStream {
    proptest::process_fn(attr, item)
}

/// Attribute macro that combines Lean verification with automatic property test generation.
///
/// This macro both generates Lean specifications AND automatically creates proptest
/// property tests that verify the Rust implementation satisfies those specifications.
///
/// # Predicate Translation
///
/// The macro translates Lean predicates to executable Rust assertions:
///
/// | Lean Pattern | Generated Rust |
/// |--------------|----------------|
/// | `∀ i j. P` | `for i in 0..len { for j in 0..len { P } }` |
/// | `P → Q` | `if P { Q }` |
/// | `a ≤ b` | `a <= b` |
/// | `P ∧ Q` | `P && Q` |
/// | `suffix_at texts sa[i]` | `suffix_at(&result.texts, &result.suffix_array[i])` |
///
/// # Attributes
///
/// - `spec = "name"` - Name for the specification and test module
/// - `requires = "predicate"` - Precondition (becomes `prop_assume!`)
/// - `ensures = "predicate"` - Postcondition (becomes assertions)
/// - `properties = ["p1", "p2"]` - Additional properties to verify
/// - `cases = 1000` - Number of proptest cases (default: 256)
/// - `strategies = [("param", "strategy")]` - Custom strategies for parameters
///
/// # Example
///
/// ```ignore
/// #[lean_proptest_verify(
///     spec = "suffix_array_sorted",
///     requires = "docs.len() > 0",
///     ensures = "∀ i j. i < j → suffix_at texts sa[i] ≤ suffix_at texts sa[j]",
///     cases = 500
/// )]
/// fn build_index(docs: Vec<SearchDoc>, texts: Vec<String>) -> SearchIndex { ... }
///
/// // Automatically generates:
/// // 1. LEAN_SPEC constant with the Lean specification
/// // 2. proptest module that:
/// //    - Generates random docs and texts
/// //    - Calls build_index
/// //    - Verifies sortedness for all pairs i < j
/// ```
///
/// # Generated Test Structure
///
/// The macro generates a test module like:
///
/// ```ignore
/// #[cfg(test)]
/// mod suffix_array_sorted_proptest {
///     proptest! {
///         #[test]
///         fn ensures_property(docs in ..., texts in ...) {
///             prop_assume!(docs.len() > 0);
///             let result = build_index(docs, texts);
///             for i in 0..result.suffix_array.len() {
///                 for j in 0..result.suffix_array.len() {
///                     if i < j {
///                         assert!(suffix_at(&result.texts, &result.suffix_array[i])
///                             <= suffix_at(&result.texts, &result.suffix_array[j]));
///                     }
///                 }
///             }
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn lean_proptest_verify(attr: TokenStream, item: TokenStream) -> TokenStream {
    proptest_gen::process(attr, item)
}
