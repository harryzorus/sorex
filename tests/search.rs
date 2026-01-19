//! Search behavior tests.

mod common;

#[path = "search/tiered.rs"]
mod tiered;

#[path = "search/correctness.rs"]
mod correctness;

#[path = "search/deduplication.rs"]
mod deduplication;

#[path = "search/ranking.rs"]
mod ranking;

#[path = "search/edge_cases.rs"]
mod edge_cases;

#[path = "search/determinism.rs"]
mod determinism;

#[path = "search/query_refinement.rs"]
mod query_refinement;

#[path = "search/tier_exclusion.rs"]
mod tier_exclusion;

#[path = "search/matched_term.rs"]
mod matched_term;
