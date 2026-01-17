// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Search algorithms: where the rubber meets the road.
//!
//! Everything culminates here. You've built indexes, computed scores,
//! precomputed DFAs. Now you actually find things. The three-tier strategy
//! (exact → prefix → fuzzy) ensures users get results fast while still
//! catching typos.

mod suffix;
pub mod dedup;
pub mod union;
pub mod tiered;
pub mod hybrid;
pub mod utils;

pub use suffix::*;
