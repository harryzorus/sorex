//! Integration tests for the build system.
//!
//! Tests the `sorex index` command workflow including:
//! - Manifest parsing
//! - Document loading
//! - Document filtering
//! - Index construction
//! - End-to-end build workflow

#[path = "build/manifest.rs"]
mod manifest;

#[path = "build/document_loading.rs"]
mod document_loading;

#[path = "build/filtering.rs"]
mod filtering;

#[path = "build/index_building.rs"]
mod index_building;

#[path = "build/e2e.rs"]
mod e2e;

#[path = "build/constants.rs"]
mod constants;
