// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! The verification layer: type-level invariants and runtime contracts.
//!
//! Two complementary approaches to catching bugs:
//!
//! 1. **Type-level wrappers** (`ValidatedSuffixEntry`, `WellFormedIndex`) that make
//!    invalid states unrepresentable. If it compiles, it satisfies the invariant.
//!
//! 2. **Runtime contracts** that panic in debug builds when invariants are violated.
//!    Zero-cost in release, but catch bugs during development.
//!
//! Use both. The type wrappers catch structural errors at compile time. The contracts
//! catch algorithmic errors when tests run.

mod types;
pub mod contracts;

pub use types::*;
