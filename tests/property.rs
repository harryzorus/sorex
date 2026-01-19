//! Property-based tests for verifying invariants.

mod common;

#[path = "property/invariants.rs"]
mod invariants;

#[path = "property/properties.rs"]
mod properties;

#[path = "property/multilingual.rs"]
mod multilingual;

#[path = "property/binary_search.rs"]
mod binary_search;

#[path = "property/fuzzy_dfa.rs"]
mod fuzzy_dfa;

#[path = "property/tiered_search.rs"]
mod tiered_search;

#[path = "property/suffix_search.rs"]
mod suffix_search;

#[path = "property/postings_encoding.rs"]
mod postings_encoding;

#[path = "property/tier_integration.rs"]
mod tier_integration;

#[path = "property/search_results.rs"]
mod search_results;

#[path = "property/oracles.rs"]
mod oracles;

#[path = "property/oracle_differential.rs"]
mod oracle_differential;

#[path = "property/suffix_array_props.rs"]
mod suffix_array_props;

#[path = "property/inverted_index_props.rs"]
mod inverted_index_props;

#[path = "property/section_props.rs"]
mod section_props;

#[path = "property/scoring_props.rs"]
mod scoring_props;

#[path = "property/binary_props.rs"]
mod binary_props;

#[path = "property/custom_scoring.rs"]
mod custom_scoring;

#[path = "property/accumulation.rs"]
mod accumulation;
