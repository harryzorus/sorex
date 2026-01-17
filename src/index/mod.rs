// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Index construction: the data structures that make search fast.
//!
//! Three index types, each optimized for different query patterns:
//! - **Suffix array**: O(log n) for any substring match, including prefixes
//! - **Inverted index**: O(1) for exact word lookup
//! - **Hybrid**: Combines both when you need everything

mod suffix_array;
mod sais;
mod inverted;
pub mod fst;
pub mod hybrid;

pub use suffix_array::*;
pub use sais::*;
pub use inverted::*;
