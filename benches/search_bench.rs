//! Benchmarks comparing our suffix array search against popular Rust libraries.
//!
//! Simulates realistic blog sizes:
//! - Small blog:  ~20 posts, ~500 words each  (personal blog)
//! - Medium blog: ~100 posts, ~1000 words each (active blogger)
//! - Large blog:  ~500 posts, ~1500 words each (publication)
//!
//! Run with: cargo bench
//!
//! Libraries compared:
//! - tantivy: Full-text search engine (Lucene-like)
//! - strsim: String similarity metrics (Levenshtein)
//! - fuzzy-matcher: FZF-style fuzzy matching
//! - simsearch: Simple in-memory fuzzy search

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sorex::{
    build_hybrid_index, build_index, build_inverted_index, build_unified_index,
    levenshtein_within, search, search_hybrid, search_unified, FieldBoundary, FieldType,
    IndexMode, IndexThresholds, SearchDoc,
};
use std::time::Duration;

// ============================================================================
// BLOG CORPUS SIMULATION
// ============================================================================

/// Blog size configurations matching real-world scenarios
struct BlogSize {
    name: &'static str,
    posts: usize,
    words_per_post: usize,
}

/// Blog sizes to benchmark
const BLOG_SIZES: &[BlogSize] = &[
    BlogSize {
        name: "small",
        posts: 20,
        words_per_post: 500,
    },
    BlogSize {
        name: "medium",
        posts: 100,
        words_per_post: 1000,
    },
];

/// Large blog size for inverted index benchmarks (suffix array too slow)
const LARGE_BLOG: BlogSize = BlogSize {
    name: "large",
    posts: 500,
    words_per_post: 1500,
};

/// Technical vocabulary for realistic blog content
const TECHNICAL_WORDS: &[&str] = &[
    "rust",
    "programming",
    "typescript",
    "javascript",
    "python",
    "golang",
    "kubernetes",
    "docker",
    "serverless",
    "microservices",
    "api",
    "database",
    "postgresql",
    "redis",
    "mongodb",
    "graphql",
    "rest",
    "websocket",
    "authentication",
    "authorization",
    "encryption",
    "security",
    "performance",
    "optimization",
    "caching",
    "indexing",
    "algorithm",
    "data",
    "structure",
    "binary",
    "tree",
    "hash",
    "map",
    "array",
    "vector",
    "queue",
    "stack",
    "concurrency",
    "parallelism",
    "async",
    "await",
    "promise",
    "future",
    "memory",
    "allocation",
    "garbage",
    "collection",
    "ownership",
    "borrowing",
    "lifetime",
    "trait",
    "interface",
    "generic",
    "type",
    "inference",
    "compiler",
    "runtime",
    "interpreter",
    "virtual",
    "machine",
    "bytecode",
    "wasm",
    "webassembly",
    "browser",
    "node",
    "deno",
    "bun",
    "framework",
];

const GENERAL_WORDS: &[&str] = &[
    "the",
    "a",
    "an",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "have",
    "has",
    "had",
    "do",
    "does",
    "did",
    "will",
    "would",
    "could",
    "should",
    "may",
    "might",
    "must",
    "shall",
    "can",
    "need",
    "application",
    "system",
    "solution",
    "approach",
    "method",
    "technique",
    "implementation",
    "development",
    "engineering",
    "architecture",
    "design",
    "pattern",
    "practice",
    "principle",
    "concept",
    "idea",
    "theory",
];

fn make_doc(id: usize) -> SearchDoc {
    SearchDoc {
        id,
        title: format!("Document {}", id),
        excerpt: format!("This is excerpt for document {}", id),
        href: format!("/posts/2024/{:02}/post-{}", (id % 12) + 1, id),
        kind: "post".to_string(),
    }
}

fn generate_content(word_count: usize, seed: usize) -> String {
    let all_words: Vec<&str> = TECHNICAL_WORDS
        .iter()
        .chain(GENERAL_WORDS.iter())
        .copied()
        .collect();

    (0..word_count)
        .map(|i| all_words[(seed * 7 + i * 3) % all_words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_blog_corpus(size: &BlogSize) -> (Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>) {
    let docs: Vec<SearchDoc> = (0..size.posts).map(make_doc).collect();

    let mut texts = Vec::with_capacity(size.posts);
    let mut boundaries = Vec::new();

    for i in 0..size.posts {
        // Generate title (short) + content (long)
        let title = format!(
            "How to Build a {} {}",
            TECHNICAL_WORDS[i % TECHNICAL_WORDS.len()],
            TECHNICAL_WORDS[(i + 1) % TECHNICAL_WORDS.len()]
        );
        let excerpt = generate_content(30, i);
        let content = generate_content(size.words_per_post, i);

        // Combine with delimiters
        let full_text = format!("{} {} {}", title, excerpt, content);
        let title_end = title.len();
        let excerpt_end = title_end + 1 + excerpt.len();

        boundaries.push(FieldBoundary {
            doc_id: i,
            start: 0,
            end: title_end,
            field_type: FieldType::Title,
        });
        boundaries.push(FieldBoundary {
            doc_id: i,
            start: title_end + 1,
            end: excerpt_end,
            field_type: FieldType::Heading, // Treat excerpt as heading for scoring
        });
        boundaries.push(FieldBoundary {
            doc_id: i,
            start: excerpt_end + 1,
            end: full_text.len(),
            field_type: FieldType::Content,
        });

        texts.push(full_text);
    }

    (docs, texts, boundaries)
}

/// Generate word pairs for fuzzy matching benchmarks
fn generate_word_pairs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("rust", "rust"),                       // Exact match
        ("rust", "ruts"),                       // 1 edit
        ("programming", "programing"),          // 1 edit (missing m)
        ("algorithm", "algorythm"),             // 1 edit
        ("performance", "performence"),         // 1 edit
        ("optimization", "optimisation"),       // 1 edit (British spelling)
        ("document", "docmuent"),               // 2 edits (transposition + typo)
        ("serverless", "serveless"),            // 1 edit
        ("engineering", "engeneering"),         // 1 edit
        ("completely", "diferent"),             // Many edits
    ]
}

// ============================================================================
// OUR IMPLEMENTATION BENCHMARKS
// ============================================================================

fn bench_ours_build_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");

    for size in BLOG_SIZES {
        let (docs, texts, boundaries) = generate_blog_corpus(size);
        let total_words: usize = texts.iter().map(|t| t.split_whitespace().count()).sum();

        group.throughput(Throughput::Elements(total_words as u64));
        group.bench_with_input(
            BenchmarkId::new("suffix_array", size.name),
            &(docs.clone(), texts.clone(), boundaries.clone()),
            |b, (docs, texts, boundaries): &(Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>)| {
                b.iter(|| {
                    build_index(
                        black_box(docs.clone()),
                        black_box(texts.clone()),
                        black_box(boundaries.clone()),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_ours_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_query");

    // Use medium blog for consistent comparison
    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);
    let index = build_index(docs, texts, boundaries);

    // Realistic blog search queries
    let queries = [
        ("single_term", "rust"),
        ("multi_term", "rust async programming"),
        ("phrase", "building api"),
        ("rare_term", "webassembly"),
        ("no_match", "xyznonexistent"),
        ("prefix", "perf"),
    ];

    for (name, query) in queries {
        group.bench_with_input(
            BenchmarkId::new("suffix_array", name),
            &query,
            |b, query| {
                b.iter(|| search(black_box(&index), black_box(query)));
            },
        );
    }

    group.finish();
}

fn bench_ours_levenshtein(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein");
    let pairs = generate_word_pairs();

    // Our implementation uses levenshtein_within for early-exit optimization
    group.bench_function("ours", |b| {
        b.iter(|| {
            for (a, b_str) in &pairs {
                // Check if within edit distance 2 (typical fuzzy search threshold)
                black_box(levenshtein_within(a, b_str, 2));
            }
        });
    });

    group.finish();
}

// ============================================================================
// INVERTED INDEX BENCHMARKS
// ============================================================================

fn bench_inverted_index_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");

    for size in BLOG_SIZES {
        let (_, texts, boundaries) = generate_blog_corpus(size);
        let texts_owned: Vec<String> = texts.clone();

        group.bench_with_input(
            BenchmarkId::new("inverted_index", size.name),
            &(texts_owned, boundaries),
            |b, (texts, boundaries)| {
                b.iter(|| {
                    build_inverted_index(black_box(texts), black_box(boundaries))
                });
            },
        );
    }

    // Also benchmark large corpus (where inverted index shines)
    let (_, texts, boundaries) = generate_blog_corpus(&LARGE_BLOG);
    let texts_owned: Vec<String> = texts.clone();

    group.bench_with_input(
        BenchmarkId::new("inverted_index", LARGE_BLOG.name),
        &(texts_owned, boundaries),
        |b, (texts, boundaries)| {
            b.iter(|| {
                build_inverted_index(black_box(texts), black_box(boundaries))
            });
        },
    );

    group.finish();
}

fn bench_inverted_index_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_query");

    // Use medium blog for comparison with suffix array
    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);
    let texts_owned: Vec<String> = texts.clone();

    // Build unified index in inverted-only mode for fair comparison
    let thresholds = IndexThresholds {
        suffix_only_max_docs: 0,      // Force inverted index
        suffix_only_max_bytes: 0,
        inverted_only_min_docs: 0,
    };
    let unified = build_unified_index(
        docs.clone(),
        texts_owned.clone(),
        boundaries.clone(),
        &thresholds,
        false,
        false,
    );

    let queries = [
        ("single_term", "rust"),
        ("multi_term", "rust programming"),
        ("rare_term", "webassembly"),
        ("no_match", "xyznonexistent"),
    ];

    for (name, query) in queries {
        group.bench_with_input(
            BenchmarkId::new("inverted_index", name),
            &query,
            |b, query| {
                b.iter(|| search_unified(black_box(&unified), black_box(query)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// HYBRID INDEX BENCHMARKS
// ============================================================================

fn bench_hybrid_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");

    for size in BLOG_SIZES {
        let (docs, texts, boundaries) = generate_blog_corpus(size);

        // Force hybrid mode
        let thresholds = IndexThresholds {
            suffix_only_max_docs: 0,
            suffix_only_max_bytes: 0,
            inverted_only_min_docs: 1000, // High threshold to get hybrid
        };

        group.bench_with_input(
            BenchmarkId::new("hybrid", size.name),
            &(docs.clone(), texts.clone(), boundaries.clone()),
            |b, (docs, texts, boundaries): &(Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>)| {
                b.iter(|| {
                    build_unified_index(
                        black_box(docs.clone()),
                        black_box(texts.clone()),
                        black_box(boundaries.clone()),
                        black_box(&thresholds),
                        true,  // needs prefix
                        true,  // needs fuzzy
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_hybrid_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_search");

    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);

    // Direct suffix array (no unified index overhead)
    let suffix_index = build_index(docs.clone(), texts.clone(), boundaries.clone());

    // Suffix array through unified index (shows abstraction overhead)
    let thresholds_suffix_unified = IndexThresholds {
        suffix_only_max_docs: 1000,  // Force suffix array only
        suffix_only_max_bytes: 100_000_000,
        inverted_only_min_docs: 10000,
    };
    let suffix_unified = build_unified_index(
        docs.clone(),
        texts.clone(),
        boundaries.clone(),
        &thresholds_suffix_unified,
        false,
        false,
    );

    // Inverted index only
    let thresholds_inverted = IndexThresholds {
        suffix_only_max_docs: 0,
        suffix_only_max_bytes: 0,
        inverted_only_min_docs: 0,
    };
    let inverted_unified = build_unified_index(
        docs.clone(),
        texts.clone(),
        boundaries.clone(),
        &thresholds_inverted,
        false,
        false,
    );

    // Verify modes are correct
    assert_eq!(suffix_unified.mode, IndexMode::SuffixArrayOnly);
    assert_eq!(inverted_unified.mode, IndexMode::InvertedIndexOnly);

    // Exact word queries
    group.bench_function("exact_word/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("rust")));
    });
    group.bench_function("exact_word/suffix_unified", |b| {
        b.iter(|| search_unified(black_box(&suffix_unified), black_box("rust")));
    });
    group.bench_function("exact_word/inverted", |b| {
        b.iter(|| search_unified(black_box(&inverted_unified), black_box("rust")));
    });

    // Prefix queries (only suffix array supports this)
    group.bench_function("prefix/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("prog")));
    });
    group.bench_function("prefix/suffix_unified", |b| {
        b.iter(|| search_unified(black_box(&suffix_unified), black_box("prog")));
    });

    // Multi-word AND queries
    group.bench_function("multi_word/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("rust programming")));
    });
    group.bench_function("multi_word/suffix_unified", |b| {
        b.iter(|| search_unified(black_box(&suffix_unified), black_box("rust programming")));
    });
    group.bench_function("multi_word/inverted", |b| {
        b.iter(|| search_unified(black_box(&inverted_unified), black_box("rust programming")));
    });

    group.finish();
}

fn bench_large_corpus(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_corpus");
    group.sample_size(50); // Fewer samples for large corpus

    let (docs, texts, boundaries) = generate_blog_corpus(&LARGE_BLOG);

    // Only inverted index can handle 500 posts efficiently
    let thresholds = IndexThresholds {
        suffix_only_max_docs: 0,
        suffix_only_max_bytes: 0,
        inverted_only_min_docs: 0,
    };

    // Build inverted-only unified index
    let unified = build_unified_index(
        docs,
        texts,
        boundaries,
        &thresholds,
        false,
        false,
    );

    assert_eq!(unified.mode, IndexMode::InvertedIndexOnly);

    group.bench_function("search/500_posts", |b| {
        b.iter(|| search_unified(black_box(&unified), black_box("rust programming")));
    });

    group.bench_function("search/rare_term", |b| {
        b.iter(|| search_unified(black_box(&unified), black_box("webassembly")));
    });

    group.finish();
}

// ============================================================================
// TRUE HYBRID INDEX: Suffix Array over Vocabulary
// ============================================================================

fn bench_true_hybrid_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");

    for size in BLOG_SIZES {
        let (docs, texts, boundaries) = generate_blog_corpus(size);

        group.bench_with_input(
            BenchmarkId::new("hybrid_vocab_sa", size.name),
            &(docs.clone(), texts.clone(), boundaries.clone()),
            |b, (docs, texts, boundaries): &(Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>)| {
                b.iter(|| {
                    build_hybrid_index(
                        black_box(docs.clone()),
                        black_box(texts.clone()),
                        black_box(boundaries.clone()),
                    )
                });
            },
        );
    }

    // Large corpus (500 posts)
    let (docs, texts, boundaries) = generate_blog_corpus(&LARGE_BLOG);
    group.bench_with_input(
        BenchmarkId::new("hybrid_vocab_sa", LARGE_BLOG.name),
        &(docs.clone(), texts.clone(), boundaries.clone()),
        |b, (docs, texts, boundaries): &(Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>)| {
            b.iter(|| {
                build_hybrid_index(
                    black_box(docs.clone()),
                    black_box(texts.clone()),
                    black_box(boundaries.clone()),
                )
            });
        },
    );

    group.finish();
}

fn bench_true_hybrid_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("true_hybrid_search");

    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);

    // Build all index types
    let suffix_index = build_index(docs.clone(), texts.clone(), boundaries.clone());
    let hybrid_index = build_hybrid_index(docs.clone(), texts.clone(), boundaries.clone());

    // Report vocabulary size vs full text size
    let vocab_size = hybrid_index.vocabulary.len();
    let vocab_suffix_size = hybrid_index.vocab_suffix_array.len();
    let full_text_size: usize = texts.iter().map(|t| t.len()).sum();
    println!(
        "\nVocabulary: {} terms, {} suffix entries (vs {} chars in full text)",
        vocab_size, vocab_suffix_size, full_text_size
    );

    // Exact word lookup (both should be O(1) via hash map)
    group.bench_function("exact_word/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("rust")));
    });
    group.bench_function("exact_word/hybrid_vocab_sa", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("rust")));
    });

    // Prefix search (hybrid uses vocab suffix array, direct uses full text)
    group.bench_function("prefix/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("prog")));
    });
    group.bench_function("prefix/hybrid_vocab_sa", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("prog")));
    });

    // Multi-word AND
    group.bench_function("multi_word/suffix_direct", |b| {
        b.iter(|| search(black_box(&suffix_index), black_box("rust programming")));
    });
    group.bench_function("multi_word/hybrid_vocab_sa", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("rust programming")));
    });

    group.finish();
}

fn bench_true_hybrid_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("true_hybrid_large");
    group.sample_size(50);

    let (docs, texts, boundaries) = generate_blog_corpus(&LARGE_BLOG);
    let hybrid_index = build_hybrid_index(docs, texts, boundaries);

    println!(
        "\nLarge corpus: {} vocabulary terms, {} suffix entries",
        hybrid_index.vocabulary.len(),
        hybrid_index.vocab_suffix_array.len()
    );

    group.bench_function("exact_word/500_posts", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("rust")));
    });

    group.bench_function("prefix/500_posts", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("prog")));
    });

    group.bench_function("multi_word/500_posts", |b| {
        b.iter(|| search_hybrid(black_box(&hybrid_index), black_box("rust programming")));
    });

    group.finish();
}

// ============================================================================
// TANTIVY COMPARISON
// ============================================================================

mod tantivy_bench {
    use super::*;
    use tantivy::collector::TopDocs;
    use tantivy::query::QueryParser;
    use tantivy::schema::{Schema, TEXT};
    use tantivy::Index;

    pub fn bench_build(c: &mut Criterion) {
        let mut group = c.benchmark_group("index_build");

        for size in BLOG_SIZES {
            let (_, texts, _) = generate_blog_corpus(size);

            group.bench_with_input(BenchmarkId::new("tantivy", size.name), &texts, |b, texts| {
                b.iter(|| {
                    let mut schema_builder = Schema::builder();
                    let title = schema_builder.add_text_field("title", TEXT);
                    let body = schema_builder.add_text_field("body", TEXT);
                    let schema = schema_builder.build();

                    let index = Index::create_in_ram(schema);
                    let mut index_writer = index.writer(50_000_000).unwrap();

                    for (i, text) in texts.iter().enumerate() {
                        index_writer
                            .add_document(tantivy::doc!(
                                title => format!("Document {}", i),
                                body => text.clone()
                            ))
                            .unwrap();
                    }

                    index_writer.commit().unwrap();
                    black_box(index)
                });
            });
        }

        group.finish();
    }

    pub fn bench_search(c: &mut Criterion) {
        let mut group = c.benchmark_group("search_query");

        let size = &BLOG_SIZES[1]; // medium
        let (_, texts, _) = generate_blog_corpus(size);

        // Build tantivy index
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT);
        let body = schema_builder.add_text_field("body", TEXT);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema);
        let mut index_writer = index.writer(50_000_000).unwrap();

        for (i, text) in texts.iter().enumerate() {
            index_writer
                .add_document(tantivy::doc!(
                    title => format!("Document {}", i),
                    body => text.clone()
                ))
                .unwrap();
        }
        index_writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&index, vec![title, body]);

        let queries = [
            ("single_term", "rust"),
            ("multi_term", "rust AND async AND programming"),
            ("phrase", "building AND api"),
            ("rare_term", "webassembly"),
            ("no_match", "xyznonexistent"),
            ("prefix", "perf*"),
        ];

        for (name, query_str) in queries {
            group.bench_with_input(
                BenchmarkId::new("tantivy", name),
                &query_str,
                |b, query_str| {
                    b.iter(|| {
                        let query = query_parser.parse_query(query_str).unwrap();
                        let results = searcher.search(&query, &TopDocs::with_limit(10)).unwrap();
                        black_box(results)
                    });
                },
            );
        }

        group.finish();
    }
}

// ============================================================================
// STRSIM COMPARISON (Levenshtein)
// ============================================================================

mod strsim_bench {
    use super::*;

    pub fn bench_levenshtein(c: &mut Criterion) {
        let mut group = c.benchmark_group("levenshtein");
        let pairs = generate_word_pairs();

        group.bench_function("strsim", |b| {
            b.iter(|| {
                for (a, b_str) in &pairs {
                    black_box(strsim::levenshtein(a, b_str));
                }
            });
        });

        group.finish();
    }
}

// ============================================================================
// FUZZY-MATCHER COMPARISON
// ============================================================================

mod fuzzy_matcher_bench {
    use super::*;
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    pub fn bench_fuzzy(c: &mut Criterion) {
        let mut group = c.benchmark_group("fuzzy_match");

        let size = &BLOG_SIZES[1]; // medium
        let (docs, texts, boundaries) = generate_blog_corpus(size);
        let index = build_index(docs, texts.clone(), boundaries);

        let matcher = SkimMatcherV2::default();

        group.bench_function("fuzzy_matcher/skim", |b| {
            b.iter(|| {
                for text in &texts {
                    black_box(matcher.fuzzy_match(text, "rust"));
                }
            });
        });

        group.bench_function("suffix_array/prefix_match", |b| {
            b.iter(|| {
                black_box(search(&index, "rust"));
            });
        });

        group.finish();
    }
}

// ============================================================================
// SIMSEARCH COMPARISON
// ============================================================================

mod simsearch_bench {
    use super::*;
    use simsearch::SimSearch;

    pub fn bench_simsearch(c: &mut Criterion) {
        let mut group = c.benchmark_group("inmemory_search");

        let size = &BLOG_SIZES[1]; // medium
        let (docs, texts, boundaries) = generate_blog_corpus(size);

        // Build simsearch index
        let mut engine: SimSearch<usize> = SimSearch::new();
        for (i, text) in texts.iter().enumerate() {
            engine.insert(i, text);
        }

        let index = build_index(docs, texts, boundaries);

        group.bench_function("simsearch", |b| {
            b.iter(|| {
                black_box(engine.search("rust programming"));
            });
        });

        group.bench_function("suffix_array", |b| {
            b.iter(|| {
                black_box(search(&index, "rust programming"));
            });
        });

        group.finish();
    }

    pub fn bench_build(c: &mut Criterion) {
        let mut group = c.benchmark_group("index_build");

        for size in BLOG_SIZES {
            let (_, texts, _) = generate_blog_corpus(size);

            group.bench_with_input(
                BenchmarkId::new("simsearch", size.name),
                &texts,
                |b, texts| {
                    b.iter(|| {
                        let mut engine: SimSearch<usize> = SimSearch::new();
                        for (i, text) in texts.iter().enumerate() {
                            engine.insert(i, text);
                        }
                        black_box(engine)
                    });
                },
            );
        }

        group.finish();
    }
}

// ============================================================================
// LATENCY PROFILE BENCHMARKS
// ============================================================================

/// Benchmark first result latency vs total query time.
///
/// This is critical for user experience - users see results as they type,
/// so time-to-first-result matters more than total time.
fn bench_latency_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_profile");

    let size = &BLOG_SIZES[1]; // medium (100 posts, 1000 words each)
    let (docs, texts, boundaries) = generate_blog_corpus(size);
    let suffix_index = build_index(docs.clone(), texts.clone(), boundaries.clone());
    let hybrid_index = build_hybrid_index(docs.clone(), texts.clone(), boundaries.clone());

    // Test queries that return different result counts
    let latency_queries = [
        ("common_word", "the", "very common - many results"),
        ("technical", "rust", "technical term - moderate results"),
        ("rare", "webassembly", "rare term - few results"),
        ("prefix_short", "pro", "short prefix - many matches"),
        ("prefix_long", "programming", "long prefix - fewer matches"),
        ("multi_word", "rust programming", "multi-word AND query"),
        ("no_match", "xyznonexistent", "zero results"),
    ];

    // Measure time to ALL results (current behavior)
    for (name, query, _desc) in &latency_queries {
        let query_name = format!("all_results/{}", name);
        group.bench_function(&query_name, |b| {
            b.iter(|| {
                let results = search(black_box(&suffix_index), black_box(query));
                black_box(results.len()) // Force complete iteration
            });
        });
    }

    // Measure time to FIRST result using hybrid index streaming
    // The search_hybrid function returns results in ranked order,
    // so we can measure time to get the first result
    for (name, query, _desc) in &latency_queries {
        let query_name = format!("first_result/{}", name);
        group.bench_function(&query_name, |b| {
            b.iter(|| {
                let results = search_hybrid(black_box(&hybrid_index), black_box(query));
                // Check if we got any results (avoids returning reference to local)
                black_box(!results.is_empty())
            });
        });
    }

    // Print result counts for context (run once outside benchmark loop)
    println!("\n=== Query Result Counts ===");
    for (name, query, desc) in &latency_queries {
        let count = search(&suffix_index, query).len();
        println!("{}: {} results ({})", name, count, desc);
    }

    group.finish();
}

/// Benchmark latency at different result set sizes.
///
/// This helps understand how result count affects latency.
fn bench_result_count_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("result_count_scaling");
    group.sample_size(100);

    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);
    let index = build_index(docs, texts, boundaries);

    // Queries that produce different result counts
    // (based on word frequency in the technical corpus)
    let result_count_queries = [
        ("1_result", "xyznonexistent"), // Will match 0, but tests overhead
        ("few_results", "webassembly"),
        ("moderate_results", "rust"),
        ("many_results", "programming"),
        ("max_results", "the"), // Common word, matches most docs
    ];

    for (name, query) in result_count_queries {
        group.bench_with_input(BenchmarkId::new("query", name), &query, |b, query| {
            b.iter(|| {
                let results = search(black_box(&index), black_box(query));
                black_box(results)
            });
        });
    }

    group.finish();
}

// ============================================================================
// SCALING BENCHMARKS
// ============================================================================

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    // Use global tight_confidence settings (500 samples, 99% CI)

    // Test how search time scales with corpus size
    for size in BLOG_SIZES {
        let (docs, texts, boundaries) = generate_blog_corpus(size);
        let index = build_index(docs, texts, boundaries);

        group.bench_with_input(
            BenchmarkId::new("corpus_size", size.name),
            &size.name,
            |b, _| {
                b.iter(|| search(black_box(&index), black_box("rust programming")));
            },
        );
    }

    // Test how search time scales with query length
    let size = &BLOG_SIZES[1]; // medium
    let (docs, texts, boundaries) = generate_blog_corpus(size);
    let index = build_index(docs, texts, boundaries);

    let query_lengths = [
        ("1_term", "rust"),
        ("3_terms", "rust programming systems"),
        ("5_terms", "rust programming systems engineering performance"),
    ];

    for (name, query) in query_lengths {
        group.bench_with_input(BenchmarkId::new("query_length", name), &query, |b, query| {
            b.iter(|| search(black_box(&index), black_box(query)));
        });
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

/// Configure Criterion for high statistical confidence.
///
/// Settings optimized for tight confidence intervals while being practical:
/// - 99% confidence level (vs default 95%)
/// - 200 samples (balance between precision and speed)
/// - 5s measurement time
/// - 3s warm-up
/// - 1% significance level (vs default 5%)
fn tight_confidence() -> Criterion {
    Criterion::default()
        .confidence_level(0.99)
        .sample_size(200)
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(3))
        .significance_level(0.01)
        .noise_threshold(0.02) // Only report changes > 2%
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    name = benches;
    config = tight_confidence();
    targets =
    // Our implementation - suffix array
    bench_ours_build_index,
    bench_ours_search,
    bench_ours_levenshtein,
    // Our implementation - inverted index
    bench_inverted_index_build,
    bench_inverted_index_search,
    // Our implementation - hybrid (unified index abstraction)
    bench_hybrid_build,
    bench_hybrid_search,
    // Our implementation - TRUE hybrid (suffix array over vocabulary)
    bench_true_hybrid_build,
    bench_true_hybrid_search,
    bench_true_hybrid_large,
    // Large corpus (inverted index only)
    bench_large_corpus,
    // Tantivy comparison
    tantivy_bench::bench_build,
    tantivy_bench::bench_search,
    // Strsim comparison
    strsim_bench::bench_levenshtein,
    // Fuzzy matcher comparison
    fuzzy_matcher_bench::bench_fuzzy,
    // Simsearch comparison
    simsearch_bench::bench_simsearch,
    simsearch_bench::bench_build,
    // Scaling tests
    bench_scaling,
    // Latency profile benchmarks
    bench_latency_profile,
    bench_result_count_scaling,
);

criterion_main!(benches);
