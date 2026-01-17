// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Scoring and ranking: how search results get their numbers.
//!
//! The key insight is that field type (title vs. heading vs. content) dominates
//! everything else. A title match at position 1000 beats a content match at
//! position 0. This is proven in Lean and enforced by the scoring constants.

mod core;
pub mod ranking;

pub use core::*;
