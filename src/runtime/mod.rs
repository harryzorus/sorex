// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Runtime bindings for WASM and Deno.
//!
//! Two flavors of the same search code: WASM for browsers and Deno for testing.
//! The WASM module is what actually runs in production. The Deno runtime exists
//! to verify that the WASM module produces identical results to native Rust.

#[cfg(feature = "wasm")]
pub mod wasm;

pub mod deno;
