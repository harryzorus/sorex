// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzzy search: typo tolerance via edit distance.
//!
//! Two implementations here: a simple bounded Levenshtein for one-off comparisons,
//! and a parametric DFA for bulk matching against many terms (the FST case).

mod levenshtein;
pub mod dfa;

pub use levenshtein::*;
