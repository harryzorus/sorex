//! Integration tests for full pipeline testing.

mod common;

#[path = "integration/index_loading.rs"]
mod index_loading;

#[path = "integration/datasets.rs"]
mod datasets;

#[path = "integration/wasm.rs"]
mod wasm;

#[path = "integration/binary_validation.rs"]
mod binary_validation;
