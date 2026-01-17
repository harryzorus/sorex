// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Utility modules for string processing, compression, and SIMD acceleration.
//!
//! This is the grab-bag of helpers that didn't fit elsewhere. Text normalization
//! for accent-insensitive search, dictionary compression for repeated strings,
//! and SIMD routines that make WASM search feel almost native.

pub mod normalize;
pub mod simd;
pub mod dict_table;
pub mod docs_compression;
