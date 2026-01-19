//! Tests for custom scoring functions via Deno.
//!
//! These tests verify that:
//! 1. TypeScript scoring files can be loaded and executed
//! 2. JavaScript scoring files work correctly
//! 3. Custom scores affect posting list ordering
//! 4. Default scoring produces expected field-type-based ranking
//!
//! Run with: cargo test --features deno-runtime test_custom_ranking

#![cfg(feature = "deno-runtime")]

use sorex::deno_runtime::{
    ScoringContext, ScoringDocContext, ScoringEvaluator, ScoringMatchContext,
};

/// Create a test scoring context for evaluation.
fn make_test_context(
    term: &str,
    field_type: &str,
    offset: usize,
    text_length: usize,
) -> ScoringContext {
    ScoringContext {
        term: term.to_string(),
        doc: ScoringDocContext {
            id: 0,
            title: "Test Document".to_string(),
            excerpt: "A test document".to_string(),
            href: "/test".to_string(),
            doc_type: "post".to_string(),
            category: Some("engineering".to_string()),
            author: Some("Test Author".to_string()),
            tags: vec!["rust".to_string(), "test".to_string()],
        },
        match_info: ScoringMatchContext {
            field_type: field_type.to_string(),
            heading_level: if field_type == "title" {
                0
            } else if field_type == "heading" {
                2
            } else {
                0
            },
            section_id: None,
            offset,
            text_length,
        },
    }
}

#[test]
fn test_default_ranking_typescript_loads() {
    let ranking_path = std::path::Path::new("tools/score.ts");
    let evaluator = ScoringEvaluator::from_file(ranking_path);
    assert!(
        evaluator.is_ok(),
        "score.ts should load: {:?}",
        evaluator.err()
    );
}

#[test]
fn test_default_ranking_field_hierarchy() {
    let ranking_path = std::path::Path::new("tools/score.ts");
    let mut evaluator =
        ScoringEvaluator::from_file(ranking_path).expect("Failed to load scoring function");

    // Create contexts for different field types at the same position
    let title_ctx = make_test_context("rust", "title", 0, 100);
    let heading_ctx = make_test_context("rust", "heading", 0, 100);
    let content_ctx = make_test_context("rust", "content", 0, 100);

    let title_score = evaluator
        .evaluate(&title_ctx)
        .expect("Title scoring failed");
    let heading_score = evaluator
        .evaluate(&heading_ctx)
        .expect("Heading scoring failed");
    let content_score = evaluator
        .evaluate(&content_ctx)
        .expect("Content scoring failed");

    // Verify field hierarchy: title > heading > content
    assert!(
        title_score > heading_score,
        "Title score ({}) should be greater than heading score ({})",
        title_score,
        heading_score
    );
    assert!(
        heading_score > content_score,
        "Heading score ({}) should be greater than content score ({})",
        heading_score,
        content_score
    );

    // Verify expected base scores from Lean-proven constants
    // TITLE = 1000, HEADING = 100, CONTENT = 10, MAX_POSITION_BONUS = 5
    assert!(title_score >= 1000, "Title base score should be >= 1000");
    assert!(
        heading_score >= 100 && heading_score < 200,
        "Heading base score should be ~100"
    );
    assert!(
        content_score >= 10 && content_score < 20,
        "Content base score should be ~10"
    );
}

#[test]
fn test_default_ranking_position_bonus() {
    let ranking_path = std::path::Path::new("tools/score.ts");
    let mut evaluator =
        ScoringEvaluator::from_file(ranking_path).expect("Failed to load scoring function");

    // Create contexts at different positions (earlier = higher bonus)
    let early_ctx = make_test_context("rust", "content", 0, 100); // start of text
    let late_ctx = make_test_context("rust", "content", 99, 100); // end of text

    let early_score = evaluator
        .evaluate(&early_ctx)
        .expect("Early position scoring failed");
    let late_score = evaluator
        .evaluate(&late_ctx)
        .expect("Late position scoring failed");

    // Earlier position should have higher score
    assert!(
        early_score > late_score,
        "Early position score ({}) should be greater than late position score ({})",
        early_score,
        late_score
    );

    // Verify the difference is within expected MAX_POSITION_BONUS range (5)
    let diff = early_score - late_score;
    assert!(
        diff <= 5,
        "Position bonus difference ({}) should be <= MAX_POSITION_BONUS (5)",
        diff
    );
}

#[test]
fn test_javascript_ranking_function() {
    // Test with plain JavaScript (no TypeScript features)
    let js_code = r#"
        function score(ctx) {
            // Simple scoring: 100 for title, 10 for heading, 1 for content
            switch (ctx.match.fieldType) {
                case "title": return 100;
                case "heading": return 10;
                default: return 1;
            }
        }
    "#;

    let mut evaluator =
        ScoringEvaluator::from_code(js_code).expect("Failed to load JS scoring function");

    let title_ctx = make_test_context("test", "title", 0, 50);
    let heading_ctx = make_test_context("test", "heading", 0, 50);
    let content_ctx = make_test_context("test", "content", 0, 50);

    let title_score = evaluator.evaluate(&title_ctx).expect("Title failed");
    let heading_score = evaluator.evaluate(&heading_ctx).expect("Heading failed");
    let content_score = evaluator.evaluate(&content_ctx).expect("Content failed");

    assert_eq!(title_score, 100, "Title should score 100");
    assert_eq!(heading_score, 10, "Heading should score 10");
    assert_eq!(content_score, 1, "Content should score 1");
}

#[test]
fn test_custom_ranking_with_document_metadata() {
    // Test that document metadata is accessible
    let js_code = r#"
        function score(ctx) {
            var s = 10;
            // Boost featured category
            if (ctx.doc.category === "engineering") {
                s += 50;
            }
            // Boost based on tags
            if (ctx.doc.tags && ctx.doc.tags.includes("rust")) {
                s += 25;
            }
            return s;
        }
    "#;

    let mut evaluator =
        ScoringEvaluator::from_code(js_code).expect("Failed to load custom scoring function");

    let ctx = make_test_context("rust", "content", 0, 50);
    let score = evaluator
        .evaluate(&ctx)
        .expect("Custom scoring with metadata failed");

    // 10 base + 50 engineering + 25 rust tag
    assert_eq!(
        score, 85,
        "Score should include metadata boosts (10 + 50 + 25 = 85)"
    );
}

#[test]
fn test_batch_evaluation() {
    let ranking_path = std::path::Path::new("tools/score.ts");
    let mut evaluator =
        ScoringEvaluator::from_file(ranking_path).expect("Failed to load scoring function");

    // Create multiple contexts
    let contexts: Vec<ScoringContext> = vec![
        make_test_context("rust", "title", 0, 100),
        make_test_context("rust", "heading", 0, 100),
        make_test_context("rust", "content", 0, 100),
        make_test_context("rust", "content", 50, 100),
        make_test_context("rust", "content", 99, 100),
    ];

    let scores = evaluator
        .evaluate_batch(&contexts)
        .expect("Batch evaluation failed");

    assert_eq!(scores.len(), 5, "Should return 5 scores");

    // Verify ordering: title > heading > content (early) > content (mid) > content (late)
    assert!(scores[0] > scores[1], "Title > heading");
    assert!(scores[1] > scores[2], "Heading > content");
    assert!(scores[2] > scores[3], "Content early > content mid");
    assert!(scores[3] > scores[4], "Content mid > content late");
}

#[test]
fn test_invalid_ranking_function_returns_error() {
    // Missing 'score' function
    let bad_code = "const x = 42;";
    let result = ScoringEvaluator::from_code(bad_code);
    assert!(
        result.is_err(),
        "Should error when no scoring function is defined"
    );

    // Non-numeric return value
    let bad_return = r#"
        function score(ctx) {
            return "not a number";
        }
    "#;
    let mut evaluator =
        ScoringEvaluator::from_code(bad_return).expect("Code should load but evaluate should fail");
    let ctx = make_test_context("test", "content", 0, 50);
    let result = evaluator.evaluate(&ctx);
    assert!(
        result.is_err(),
        "Should error when scoring function returns non-number"
    );
}

#[test]
fn test_ranking_preserves_proven_invariants() {
    // Verify Lean-proven invariant: Title - MaxBoost > Heading + MaxBoost
    let ranking_path = std::path::Path::new("tools/score.ts");
    let mut evaluator =
        ScoringEvaluator::from_file(ranking_path).expect("Failed to load scoring function");

    // Test with worst-case position bonuses (title at end, heading at start)
    let title_worst = make_test_context("rust", "title", 99, 100); // minimal position bonus
    let heading_best = make_test_context("rust", "heading", 0, 100); // maximal position bonus

    let title_score = evaluator
        .evaluate(&title_worst)
        .expect("Title scoring failed");
    let heading_score = evaluator
        .evaluate(&heading_best)
        .expect("Heading scoring failed");

    // Even with worst title position and best heading position,
    // title should still dominate (Lean theorem: title_beats_heading)
    assert!(
        title_score > heading_score,
        "Title at worst position ({}) should still beat heading at best position ({})",
        title_score,
        heading_score
    );

    // Similarly for heading vs content
    let heading_worst = make_test_context("rust", "heading", 99, 100);
    let content_best = make_test_context("rust", "content", 0, 100);

    let heading_worst_score = evaluator
        .evaluate(&heading_worst)
        .expect("Heading scoring failed");
    let content_best_score = evaluator
        .evaluate(&content_best)
        .expect("Content scoring failed");

    assert!(
        heading_worst_score > content_best_score,
        "Heading at worst position ({}) should still beat content at best position ({})",
        heading_worst_score,
        content_best_score
    );
}
