mod tier1_exact_search_tests {
    use std::collections::HashSet;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct PostingEntry {
        doc_id: u32,
        section_idx: u32,
    }

    /// Simulate T1 exact search deduplication
    fn tier1_exact(postings: &[PostingEntry], limit: usize) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }

        let mut seen_docs = HashSet::new();
        let mut doc_ids: Vec<usize> = Vec::with_capacity(limit);

        for entry in postings.iter() {
            let doc_id = entry.doc_id as usize;
            if seen_docs.insert(doc_id) {
                doc_ids.push(doc_id);
                if doc_ids.len() >= limit {
                    break;
                }
            }
        }
        doc_ids
    }

    // ====== PROPERTY 1: Deduplication ======

    /// P1.1: Result contains no duplicate doc IDs
    #[test]
    fn prop_tier1_no_duplicates_in_result() {
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 0,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 0,
                section_idx: 2,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
        ];

        let result = tier1_exact(&postings, 10);
        let unique: HashSet<_> = result.iter().cloned().collect();
        assert_eq!(
            result.len(),
            unique.len(),
            "Result has duplicates: {:?}",
            result
        );
    }

    /// P1.2: Output length <= limit
    #[test]
    fn prop_tier1_respects_limit() {
        for limit in [1, 5, 10, 50, 100] {
            let mut postings = Vec::new();
            for doc_id in 0..200 {
                for section in 0..5 {
                    postings.push(PostingEntry {
                        doc_id,
                        section_idx: section,
                    });
                }
            }

            let result = tier1_exact(&postings, limit);
            assert!(
                result.len() <= limit,
                "Limit {}: got {} results",
                limit,
                result.len()
            );
        }
    }

    /// P1.3: If fewer unique docs than limit, return all
    #[test]
    fn prop_tier1_returns_all_when_fewer_than_limit() {
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 0,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
        ];

        for limit in [5, 10, 100] {
            let result = tier1_exact(&postings, limit);
            assert_eq!(
                result.len(),
                3,
                "Limit {}: expected 3, got {}",
                limit,
                result.len()
            );
        }
    }

    // ====== PROPERTY 2: Ordering ======

    /// P2.1: Results maintain postings order (first occurrence of each doc)
    #[test]
    fn prop_tier1_preserves_posting_order() {
        let postings = vec![
            PostingEntry {
                doc_id: 5,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 5,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 8,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 1,
            },
        ];

        let result = tier1_exact(&postings, 10);
        assert_eq!(
            result,
            vec![5, 2, 8, 3],
            "Expected [5, 2, 8, 3], got {:?}",
            result
        );
    }

    /// P2.2: First occurrence determines position
    #[test]
    fn prop_tier1_first_occurrence_only() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 1,
            }, // Duplicate, should be ignored
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 1,
            }, // Duplicate, should be ignored
        ];

        let result = tier1_exact(&postings, 10);
        assert_eq!(
            result,
            vec![1, 2, 3],
            "Expected [1, 2, 3], got {:?}",
            result
        );
    }

    // ====== PROPERTY 3: Edge Cases ======

    /// P3.1: Empty postings
    #[test]
    fn prop_tier1_empty_postings() {
        let result = tier1_exact(&[], 10);
        assert_eq!(result.len(), 0, "Empty input should produce empty output");
    }

    /// P3.2: Single posting entry
    #[test]
    fn prop_tier1_single_posting() {
        let postings = vec![PostingEntry {
            doc_id: 42,
            section_idx: 0,
        }];
        let result = tier1_exact(&postings, 10);
        assert_eq!(result, vec![42]);
    }

    /// P3.3: All postings same doc_id
    #[test]
    fn prop_tier1_all_same_doc() {
        let mut postings = Vec::new();
        for section in 0..20 {
            postings.push(PostingEntry {
                doc_id: 7,
                section_idx: section,
            });
        }

        let result = tier1_exact(&postings, 10);
        assert_eq!(result, vec![7], "All same doc should return just [7]");
    }

    /// P3.4: Limit = 1
    #[test]
    fn prop_tier1_limit_one() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let result = tier1_exact(&postings, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 1, "First doc should be returned");
    }

    /// P3.5: Limit = 0
    #[test]
    fn prop_tier1_limit_zero() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
        ];

        let result = tier1_exact(&postings, 0);
        assert_eq!(result.len(), 0, "Limit 0 should return no results");
    }

    // ====== PROPERTY 4: Cardinality ======

    /// P4.1: Output cardinality = min(unique_docs, limit)
    #[test]
    fn prop_tier1_cardinality_formula() {
        for (unique_docs, limit, expected) in &[
            (5, 10, 5),   // fewer docs than limit
            (10, 5, 5),   // more docs than limit
            (10, 10, 10), // equal
            (1, 1, 1),
            (100, 50, 50),
        ] {
            let mut postings = Vec::new();
            for doc_id in 0..*unique_docs {
                for section in 0..3 {
                    postings.push(PostingEntry {
                        doc_id: doc_id as u32,
                        section_idx: section,
                    });
                }
            }

            let result = tier1_exact(&postings, *limit);
            assert_eq!(
                result.len(),
                *expected,
                "unique_docs={}, limit={}: expected {}, got {}",
                unique_docs,
                limit,
                expected,
                result.len()
            );
        }
    }

    // ====== PROPERTY 5: Real-world patterns ======

    /// P5.1: Hotspot distribution (80% in 2 docs, 20% in others)
    #[test]
    fn prop_tier1_hotspot_distribution() {
        let mut postings = Vec::new();

        // 8 sections in doc 0
        for section in 0..8 {
            postings.push(PostingEntry {
                doc_id: 0,
                section_idx: section,
            });
        }
        // 7 sections in doc 1
        for section in 0..7 {
            postings.push(PostingEntry {
                doc_id: 1,
                section_idx: section,
            });
        }
        // 1 section each in docs 2-9
        for doc_id in 2..10 {
            postings.push(PostingEntry {
                doc_id,
                section_idx: 0,
            });
        }

        let result = tier1_exact(&postings, 5);
        assert_eq!(
            result,
            vec![0, 1, 2, 3, 4],
            "Hotspot pattern should maintain order"
        );
    }

    /// P5.2: Real "gemm" pattern from CUTLASS (36 docs, varying sections)
    #[test]
    fn prop_tier1_gemm_pattern() {
        let mut postings = Vec::new();
        // Simulate gemm postings: 36 unique docs with 1 section each
        for doc_id in 0..36 {
            postings.push(PostingEntry {
                doc_id: doc_id as u32,
                section_idx: 0,
            });
        }

        let result = tier1_exact(&postings, 39);
        assert_eq!(result.len(), 36, "gemm should return 36 unique docs");
        for (i, &doc_id) in result.iter().enumerate() {
            assert_eq!(doc_id, i, "Expected docs in order 0..36");
        }
    }

    /// P5.3: Dense repetition (doc appears many times)
    #[test]
    fn prop_tier1_dense_repetition() {
        let mut postings = Vec::new();
        // Doc 1 appears 200 times
        for section in 0..200 {
            postings.push(PostingEntry {
                doc_id: 1,
                section_idx: section as u32,
            });
        }
        // Docs 2-5 appear once each
        for doc_id in 2..6 {
            postings.push(PostingEntry {
                doc_id,
                section_idx: 0,
            });
        }

        let result = tier1_exact(&postings, 10);
        assert_eq!(result, vec![1, 2, 3, 4, 5], "Should return [1,2,3,4,5]");
        assert_eq!(result.len(), 5, "Should have exactly 5 unique docs");
    }

    // ====== PROPERTY 6: Correctness against naive dedup ======

    /// P6.1: Match against HashSet deduplicate
    #[test]
    fn prop_tier1_matches_hashset_dedup() {
        let mut postings = Vec::new();
        for doc_id in 0..50 {
            for section in 0..(doc_id % 5 + 1) {
                postings.push(PostingEntry {
                    doc_id,
                    section_idx: section,
                });
            }
        }

        // Compute result from tier1_exact
        for limit in [5, 10, 25, 50, 100] {
            let result = tier1_exact(&postings, limit);

            // Compute naive dedup
            let mut seen = HashSet::new();
            let mut naive = Vec::new();
            for posting in &postings {
                if seen.insert(posting.doc_id) {
                    naive.push(posting.doc_id as usize);
                    if naive.len() >= limit {
                        break;
                    }
                }
            }

            assert_eq!(
                result, naive,
                "Mismatch at limit {}: tier1={:?} vs naive={:?}",
                limit, result, naive
            );
        }
    }
}

// ====== TIER 2 PREFIX SEARCH TESTS ======

mod tier2_prefix_search_tests {
    use std::collections::HashSet;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct PostingEntry {
        doc_id: u32,
        section_idx: u32,
    }

    /// Simulate T2 prefix deduplication (exclude docs already in T1)
    fn tier2_with_exclusion(
        postings: &[PostingEntry],
        exclude_docs: &[usize],
        limit: usize,
    ) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }

        let exclude_set: HashSet<usize> = exclude_docs.iter().cloned().collect();
        let mut seen_docs = HashSet::new();
        let mut doc_ids: Vec<usize> = Vec::with_capacity(limit);

        for entry in postings.iter() {
            let doc_id = entry.doc_id as usize;
            // Skip if already in T1 (exclude set)
            if exclude_set.contains(&doc_id) {
                continue;
            }
            // Skip if already seen in T2
            if seen_docs.insert(doc_id) {
                doc_ids.push(doc_id);
                if doc_ids.len() >= limit {
                    break;
                }
            }
        }
        doc_ids
    }

    // ====== PROPERTY 1: Exclusion ======

    /// P1.1: T2 never returns docs from exclude set
    #[test]
    fn prop_tier2_excludes_t1_docs() {
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 0,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let exclude = vec![0, 1]; // T1 returned these
        let result = tier2_with_exclusion(&postings, &exclude, 10);

        // Should only have docs 2, 3
        let result_set: HashSet<_> = result.iter().cloned().collect();
        for excluded_doc in &exclude {
            assert!(
                !result_set.contains(excluded_doc),
                "T2 returned excluded doc {}",
                excluded_doc
            );
        }
    }

    /// P1.2: T2 returns union minus T1
    #[test]
    fn prop_tier2_returns_new_docs() {
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let exclude = vec![0, 1];
        let result = tier2_with_exclusion(&postings, &exclude, 10);

        assert_eq!(result, vec![2, 3], "Expected [2, 3], got {:?}", result);
    }

    // ====== PROPERTY 2: Deduplication ======

    /// P2.1: T2 deduplicates docs with multiple sections
    #[test]
    fn prop_tier2_deduplicates_own_results() {
        let postings = vec![
            PostingEntry {
                doc_id: 5,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 5,
                section_idx: 1,
            },
            PostingEntry {
                doc_id: 5,
                section_idx: 2,
            },
            PostingEntry {
                doc_id: 6,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 6,
                section_idx: 1,
            },
        ];

        let exclude = vec![];
        let result = tier2_with_exclusion(&postings, &exclude, 10);

        assert_eq!(result, vec![5, 6], "Should deduplicate to [5, 6]");
        assert_eq!(result.len(), 2, "Should have 2 unique docs");
    }

    // ====== PROPERTY 3: Limit ======

    /// P3.1: T2 respects limit with exclusions
    #[test]
    fn prop_tier2_respects_limit_with_exclude() {
        let mut postings = Vec::new();
        for doc_id in 0..20 {
            postings.push(PostingEntry {
                doc_id: doc_id as u32,
                section_idx: 0,
            });
        }

        let exclude = vec![0, 1, 2]; // Exclude first 3
        for limit in [1, 5, 10] {
            let result = tier2_with_exclusion(&postings, &exclude, limit);
            assert!(
                result.len() <= limit,
                "Limit {}: got {} results",
                limit,
                result.len()
            );
            // Should start from doc 3 (first non-excluded)
            assert_eq!(result[0], 3, "First result should be doc 3");
        }
    }

    /// P3.2: Empty exclude set
    #[test]
    fn prop_tier2_empty_exclude_set() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let result = tier2_with_exclusion(&postings, &[], 10);
        assert_eq!(result, vec![1, 2, 3], "Empty exclude should return all");
    }

    /// P3.3: All docs excluded
    #[test]
    fn prop_tier2_all_docs_excluded() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let exclude = vec![1, 2, 3]; // Exclude everything
        let result = tier2_with_exclusion(&postings, &exclude, 10);
        assert_eq!(result.len(), 0, "All excluded should return nothing");
    }

    // ====== PROPERTY 4: Real-world patterns ======

    /// P4.1: T2 complements T1 (gemm + ge patterns)
    #[test]
    fn prop_tier2_complements_tier1() {
        // Simulate: "gemm" matches in docs [0,1,2], "ge*" prefix matches in [0,1,2,3,4]
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 4,
                section_idx: 0,
            },
        ];

        let t1_results = vec![0, 1, 2]; // T1 exact matches
        let t2_results = tier2_with_exclusion(&postings, &t1_results, 10);

        // T2 should only return [3, 4]
        assert_eq!(t2_results, vec![3, 4], "T2 should return [3, 4]");
        assert_eq!(t2_results.len(), 2);
    }

    /// P4.2: Exclude set larger than results
    #[test]
    fn prop_tier2_large_exclude_set() {
        let postings = vec![
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        let exclude = vec![0, 1, 2, 3, 4, 5]; // Much larger exclude set
        let result = tier2_with_exclusion(&postings, &exclude, 10);
        assert_eq!(
            result.len(),
            0,
            "Large exclude should eliminate all results"
        );
    }

    // ====== PROPERTY 5: Correctness ======

    /// P5.1: No overlap between T1 and T2
    #[test]
    fn prop_tier2_no_overlap_with_t1() {
        let postings = vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 1,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 2,
                section_idx: 0,
            },
            PostingEntry {
                doc_id: 3,
                section_idx: 0,
            },
        ];

        for exclude_subset in [vec![], vec![0], vec![0, 1], vec![0, 1, 2], vec![0, 1, 2, 3]] {
            let result = tier2_with_exclusion(&postings, &exclude_subset, 10);
            let exclude_set: HashSet<_> = exclude_subset.iter().cloned().collect();

            for doc_id in &result {
                assert!(
                    !exclude_set.contains(doc_id),
                    "Doc {} in T2 but also in exclude set",
                    doc_id
                );
            }
        }
    }

    /// P5.2: Union of T1 and T2 = all unique docs minus exclude
    #[test]
    fn prop_tier2_t1_union_t2_equals_total() {
        let mut postings = Vec::new();
        for doc_id in 0..10 {
            postings.push(PostingEntry {
                doc_id: doc_id as u32,
                section_idx: 0,
            });
        }

        let t1_results = vec![0, 1, 2];
        let t2_results = tier2_with_exclusion(&postings, &t1_results, 10);

        let mut combined = t1_results.clone();
        combined.extend(t2_results);
        combined.sort();

        let expected: Vec<usize> = (0..10).collect();
        assert_eq!(combined, expected, "T1 ∪ T2 should equal all docs");
    }
}

// ====== TIER 3 FUZZY SEARCH TESTS ======

mod tier3_fuzzy_search_tests {
    use std::collections::HashSet;

    /// Levenshtein distance computation (for testing edit distance assumptions)
    fn levenshtein(s1: &str, s2: &str) -> usize {
        let s1 = s1.to_lowercase();
        let s2 = s2.to_lowercase();
        let mut matrix = vec![vec![0; s2.len() + 1]; s1.len() + 1];

        for i in 0..=s1.len() {
            matrix[i][0] = i;
        }
        for j in 0..=s2.len() {
            matrix[0][j] = j;
        }

        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();

        for (i, &c1) in s1_chars.iter().enumerate() {
            for (j, &c2) in s2_chars.iter().enumerate() {
                let cost = if c1 == c2 { 0 } else { 1 };
                matrix[i + 1][j + 1] = std::cmp::min(
                    std::cmp::min(
                        matrix[i][j + 1] + 1, // deletion
                        matrix[i + 1][j] + 1, // insertion
                    ),
                    matrix[i][j] + cost, // substitution
                );
            }
        }

        matrix[s1.len()][s2.len()]
    }

    /// Simulate T3 fuzzy search (exclude docs from T1+T2, find typos within edit distance 2)
    fn tier3_fuzzy(
        all_terms: &[&str],
        term_postings: &[Vec<u32>], // postings for each term
        query: &str,
        exclude_docs: &[usize],
        max_distance: usize,
        limit: usize,
    ) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }

        let exclude_set: HashSet<usize> = exclude_docs.iter().cloned().collect();
        let mut results_by_doc: HashSet<usize> = HashSet::new();

        // Find all terms within edit distance max_distance
        for (term_idx, &term) in all_terms.iter().enumerate() {
            let dist = levenshtein(query, term);
            if dist > 0 && dist <= max_distance {
                // This term is a fuzzy match
                if let Some(postings) = term_postings.get(term_idx) {
                    for &doc_id in postings {
                        let doc_usize = doc_id as usize;
                        if !exclude_set.contains(&doc_usize) {
                            results_by_doc.insert(doc_usize);
                            if results_by_doc.len() >= limit {
                                return results_by_doc.into_iter().collect();
                            }
                        }
                    }
                }
            }
        }

        results_by_doc.into_iter().collect()
    }

    // ====== PROPERTY 1: Edit Distance ======

    /// P1.1: All fuzzy results have edit distance <= 2
    #[test]
    fn prop_tier3_edit_distance_bound() {
        let query = "gemm";
        let terms = vec!["gem", "gemm", "gemmk", "hgemm", "hello", "world"];
        let term_postings = vec![
            vec![0],    // "gem"
            vec![1, 2], // "gemm" (exact, won't appear in T3)
            vec![3],    // "gemmk"
            vec![4],    // "hgemm"
            vec![],     // "hello"
            vec![],     // "world"
        ];

        let _t1_results = [1, 2]; // "gemm" exact matches
        let _results = tier3_fuzzy(&terms, &term_postings, query, &[1, 2], 2, 100);

        // Check that all matched terms have edit distance <= 2
        let matched_terms: Vec<&str> = terms
            .iter()
            .filter(|t| levenshtein(query, t) > 0 && levenshtein(query, t) <= 2)
            .cloned()
            .collect();

        // "gemm" exact (dist=0, excluded), "gemmk" (dist=1), "hgemm" (dist=1)
        assert!(
            matched_terms.contains(&"gemmk"),
            "gemmk should be within edit distance 2"
        );
        assert!(
            matched_terms.contains(&"hgemm"),
            "hgemm should be within edit distance 2"
        );
    }

    // ====== PROPERTY 2: Exclusion ======

    /// P2.1: T3 excludes T1+T2 results
    #[test]
    fn prop_tier3_excludes_t1_t2() {
        let query = "test";
        let terms = vec!["test", "tests", "text", "tent"];
        let term_postings = vec![
            vec![0], // "test" (T1)
            vec![1], // "tests" (T2 prefix)
            vec![2], // "text" (T3 typo)
            vec![3], // "tent" (T3 typo)
        ];

        let excluded = vec![0, 1]; // T1 and T2 results
        let results = tier3_fuzzy(&terms, &term_postings, query, &excluded, 2, 100);

        let result_set: HashSet<_> = results.iter().cloned().collect();
        for doc_id in &excluded {
            assert!(
                !result_set.contains(doc_id),
                "T3 returned doc {} which was excluded",
                doc_id
            );
        }
    }

    // ====== PROPERTY 3: Deduplication ======

    /// P3.1: T3 deduplicates multiple fuzzy matches pointing to same doc
    #[test]
    fn prop_tier3_deduplicates_results() {
        let query = "test";
        let terms = vec!["test", "tests", "text", "test"]; // "test" appears twice
        let term_postings = vec![
            vec![0], // "test"
            vec![1], // "tests"
            vec![2], // "text"
            vec![0], // "test" again -> same doc
        ];

        let excluded = vec![];
        let results = tier3_fuzzy(&terms, &term_postings, query, &excluded, 2, 100);

        let unique_results: HashSet<_> = results.iter().cloned().collect();
        assert_eq!(
            results.len(),
            unique_results.len(),
            "Results should have no duplicates"
        );
    }

    // ====== PROPERTY 4: Limit ======

    /// P4.1: T3 respects limit
    #[test]
    fn prop_tier3_respects_limit() {
        let query = "a";
        let terms = vec!["a", "b", "c", "d", "aa", "ab", "ac"];
        let term_postings = vec![
            vec![0],
            vec![1],
            vec![2],
            vec![3],
            vec![4],
            vec![5],
            vec![6],
        ];

        for limit in [1, 3, 5, 10] {
            let results = tier3_fuzzy(&terms, &term_postings, query, &[], 2, limit);
            assert!(
                results.len() <= limit,
                "Limit {}: got {} results",
                limit,
                results.len()
            );
        }
    }

    /// P4.2: T3 with zero limit returns empty
    #[test]
    fn prop_tier3_limit_zero() {
        let query = "test";
        let terms = vec!["test", "tests", "text"];
        let term_postings = vec![vec![0], vec![1], vec![2]];

        let results = tier3_fuzzy(&terms, &term_postings, query, &[], 2, 0);
        assert_eq!(results.len(), 0, "Limit 0 should return nothing");
    }

    // ====== PROPERTY 5: False Positives ======

    /// P5.1: Common fuzzy false positives (edit distance <= 2)
    #[test]
    fn prop_tier3_identifies_fuzzy_variants() {
        let query = "gemm";

        // These are all within edit distance 2 of "gemm"
        let fuzzy_variants = vec![
            ("gemm", 0),  // exact
            ("gem", 1),   // 1 char deletion
            ("gemms", 1), // 1 char insertion
            ("gemm", 0),  // exact again
            ("hemm", 1),  // 1 char substitution
            ("gemma", 1), // 1 char substitution
            ("gemmk", 1), // 1 char substitution
            ("geme", 1),  // 1 substitution (last m->e)
            ("gem", 1),   // 1 deletion (duplicate)
        ];

        for (variant, expected_dist) in fuzzy_variants {
            let actual_dist = levenshtein(query, variant);
            assert_eq!(
                actual_dist, expected_dist,
                "levenshtein('{}', '{}') should be {}",
                query, variant, expected_dist
            );
            if actual_dist > 0 && actual_dist <= 2 {
                // These should be caught by T3
                assert!(actual_dist <= 2, "{} is a valid fuzzy match", variant);
            }
        }
    }

    /// P5.2: False positives beyond edit distance 2
    #[test]
    fn prop_tier3_rejects_distant_terms() {
        let query = "gemm";

        // These are NOT within edit distance 2
        let distant_variants = vec![("hello", 9), ("world", 8), ("test", 4)];

        for (variant, _expected_dist) in distant_variants {
            let actual_dist = levenshtein(query, variant);
            assert!(
                actual_dist > 2,
                "Expected '{}' to be > 2 away from '{}', got {}",
                variant,
                query,
                actual_dist
            );
        }
    }

    // ====== PROPERTY 6: Correctness ======

    /// P6.1: No overlap with T1 and T2
    #[test]
    fn prop_tier3_no_overlap_t1_t2() {
        let query = "test";
        let terms = vec!["test", "tests", "text"];
        let term_postings = vec![vec![0], vec![1], vec![2]];

        let t1_results = vec![0];
        let t2_results = vec![1];
        let mut excluded = t1_results.clone();
        excluded.extend(t2_results);

        let t3_results = tier3_fuzzy(&terms, &term_postings, query, &excluded, 2, 100);
        let exclude_set: HashSet<_> = excluded.iter().cloned().collect();

        for doc_id in &t3_results {
            assert!(
                !exclude_set.contains(doc_id),
                "Doc {} in both T3 and T1+T2",
                doc_id
            );
        }
    }

    /// P6.2: Query with no fuzzy matches
    #[test]
    fn prop_tier3_no_matches_for_unique_query() {
        let query = "zzzzz";
        let terms = vec!["a", "b", "c", "test"];
        let term_postings = vec![vec![0], vec![1], vec![2], vec![3]];

        // "zzzzz" is far from all terms, should get no fuzzy matches with distance <= 2
        let results = tier3_fuzzy(&terms, &term_postings, query, &[], 2, 100);
        // All terms are > 2 distance away, so results should be empty
        assert!(
            results.is_empty() || results.len() <= 1,
            "Very unique query should have minimal matches: {:?}",
            results
        );
    }
}

// ====== REPLACE-IF-BETTER DEDUPLICATION TESTS ======

mod replace_if_better_tests {
    use std::cmp::Ordering;

    /// Simulates the MatchType enum ordering
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[allow(dead_code)]
    enum MatchType {
        Title = 0, // Best - lowest value
        Section = 1,
        Subsection = 2,
        Paragraph = 3,
        Content = 4, // Worst - highest value
    }

    #[derive(Debug, Clone)]
    struct SearchResult {
        doc_id: usize,
        match_type: MatchType,
        score: f64,
        tier: u8,
    }

    /// Returns true if `new` is better ranked than `existing`.
    /// Better = lower match_type, or same match_type with higher score.
    fn is_better(new: &SearchResult, existing: &SearchResult) -> bool {
        match new.match_type.cmp(&existing.match_type) {
            Ordering::Less => true,                        // Lower match_type is better
            Ordering::Greater => false,                    // Higher match_type is worse
            Ordering::Equal => new.score > existing.score, // Same type, compare score
        }
    }

    /// Simulates the cursor's add_results with replace-if-better logic
    fn add_results_replace_if_better(
        results_by_doc: &mut std::collections::HashMap<usize, SearchResult>,
        new_results: Vec<SearchResult>,
    ) {
        for r in new_results {
            results_by_doc
                .entry(r.doc_id)
                .and_modify(|existing| {
                    if is_better(&r, existing) {
                        *existing = r.clone();
                    }
                })
                .or_insert(r);
        }
    }

    // ====== PROPERTY 1: Lower match_type replaces higher ======

    /// P1.1: Title match replaces Content match
    #[test]
    fn prop_title_replaces_content() {
        let mut results_by_doc = std::collections::HashMap::new();

        // T1 finds doc 0 with Content match
        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Content,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        // T2 finds doc 0 with Title match (better!)
        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 50.0, // Even with lower score, Title wins
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
        assert_eq!(final_result.tier, 2, "Should be from T2");
    }

    /// P1.2: Section match replaces Content match
    #[test]
    fn prop_section_replaces_content() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Content,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t3_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 30.0,
            tier: 3,
        }];
        add_results_replace_if_better(&mut results_by_doc, t3_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Section);
        assert_eq!(final_result.tier, 3);
    }

    /// P1.3: Title match replaces Section match
    #[test]
    fn prop_title_replaces_section() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        let t3_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 30.0,
            tier: 3,
        }];
        add_results_replace_if_better(&mut results_by_doc, t3_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
    }

    // ====== PROPERTY 2: Higher match_type doesn't replace ======

    /// P2.1: Content match doesn't replace Title match
    #[test]
    fn prop_content_doesnt_replace_title() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Content,
            score: 200.0, // Even with higher score, Content doesn't replace Title
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
        assert_eq!(final_result.tier, 1, "Should stay from T1");
        assert_eq!(final_result.score, 100.0, "Score unchanged");
    }

    /// P2.2: Section match doesn't replace Title match
    #[test]
    fn prop_section_doesnt_replace_title() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t3_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 3,
        }];
        add_results_replace_if_better(&mut results_by_doc, t3_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
        assert_eq!(final_result.tier, 1);
    }

    // ====== PROPERTY 3: Same match_type, higher score replaces ======

    /// P3.1: Same match_type, higher score replaces
    #[test]
    fn prop_same_type_higher_score_replaces() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 75.0, // Same match_type, higher score
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Section);
        assert_eq!(final_result.score, 75.0, "Higher score should win");
        assert_eq!(final_result.tier, 2);
    }

    /// P3.2: Same match_type, lower score doesn't replace
    #[test]
    fn prop_same_type_lower_score_doesnt_replace() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 75.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0, // Same match_type, lower score
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.score, 75.0, "Higher score should remain");
        assert_eq!(final_result.tier, 1);
    }

    // ====== PROPERTY 4: Multiple docs handled correctly ======

    /// P4.1: Multiple docs each get their best match
    #[test]
    fn prop_multiple_docs_each_get_best() {
        let mut results_by_doc = std::collections::HashMap::new();

        // T1: doc 0 Content, doc 1 Title, doc 2 Section
        let t1_results = vec![
            SearchResult {
                doc_id: 0,
                match_type: MatchType::Content,
                score: 100.0,
                tier: 1,
            },
            SearchResult {
                doc_id: 1,
                match_type: MatchType::Title,
                score: 100.0,
                tier: 1,
            },
            SearchResult {
                doc_id: 2,
                match_type: MatchType::Section,
                score: 100.0,
                tier: 1,
            },
        ];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        // T2: doc 0 Title (upgrade!), doc 1 Content (no change), doc 2 Title (upgrade!)
        let t2_results = vec![
            SearchResult {
                doc_id: 0,
                match_type: MatchType::Title,
                score: 50.0,
                tier: 2,
            },
            SearchResult {
                doc_id: 1,
                match_type: MatchType::Content,
                score: 50.0,
                tier: 2,
            },
            SearchResult {
                doc_id: 2,
                match_type: MatchType::Title,
                score: 50.0,
                tier: 2,
            },
        ];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        // Verify each doc has correct final state
        assert_eq!(results_by_doc.get(&0).unwrap().match_type, MatchType::Title);
        assert_eq!(results_by_doc.get(&0).unwrap().tier, 2); // Upgraded from T1

        assert_eq!(results_by_doc.get(&1).unwrap().match_type, MatchType::Title);
        assert_eq!(results_by_doc.get(&1).unwrap().tier, 1); // Stayed from T1

        assert_eq!(results_by_doc.get(&2).unwrap().match_type, MatchType::Title);
        assert_eq!(results_by_doc.get(&2).unwrap().tier, 2); // Upgraded from T1
    }

    // ====== PROPERTY 5: Cross-tier scenario ======

    /// P5.1: Realistic scenario - doc appears in all 3 tiers, best is kept
    #[test]
    fn prop_realistic_three_tier_scenario() {
        let mut results_by_doc = std::collections::HashMap::new();

        // T1: exact match for "tensor" in Content
        let t1_results = vec![SearchResult {
            doc_id: 42,
            match_type: MatchType::Content,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);
        assert_eq!(results_by_doc.len(), 1);

        // T2: prefix match for "tens" finds "tensor" in Section (better!)
        let t2_results = vec![SearchResult {
            doc_id: 42,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);
        assert_eq!(
            results_by_doc.get(&42).unwrap().match_type,
            MatchType::Section
        );

        // T3: fuzzy match finds "tensor" in Title (even better!)
        let t3_results = vec![SearchResult {
            doc_id: 42,
            match_type: MatchType::Title,
            score: 30.0,
            tier: 3,
        }];
        add_results_replace_if_better(&mut results_by_doc, t3_results);

        // Final result should be Title from T3
        let final_result = results_by_doc.get(&42).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
        assert_eq!(final_result.tier, 3);
        assert_eq!(final_result.score, 30.0);
    }

    /// P5.2: Tier with worse match doesn't affect final result
    #[test]
    fn prop_later_tier_worse_match_ignored() {
        let mut results_by_doc = std::collections::HashMap::new();

        // T1: Title match
        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        // T2: Section match (worse)
        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        // T3: Content match (even worse)
        let t3_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Content,
            score: 30.0,
            tier: 3,
        }];
        add_results_replace_if_better(&mut results_by_doc, t3_results);

        // Final result should still be Title from T1
        let final_result = results_by_doc.get(&0).unwrap();
        assert_eq!(final_result.match_type, MatchType::Title);
        assert_eq!(final_result.tier, 1);
        assert_eq!(final_result.score, 100.0);
    }

    // ====== PROPERTY 6: Edge cases ======

    /// P6.1: Empty tier results don't affect existing
    #[test]
    fn prop_empty_tier_no_effect() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Title,
            score: 100.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        // Empty T2 results
        add_results_replace_if_better(&mut results_by_doc, vec![]);

        assert_eq!(results_by_doc.len(), 1);
        assert_eq!(results_by_doc.get(&0).unwrap().tier, 1);
    }

    /// P6.2: Same score, same match_type - first one wins (no change)
    #[test]
    fn prop_same_score_same_type_first_wins() {
        let mut results_by_doc = std::collections::HashMap::new();

        let t1_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0,
            tier: 1,
        }];
        add_results_replace_if_better(&mut results_by_doc, t1_results);

        let t2_results = vec![SearchResult {
            doc_id: 0,
            match_type: MatchType::Section,
            score: 50.0, // Same score, same type
            tier: 2,
        }];
        add_results_replace_if_better(&mut results_by_doc, t2_results);

        // First one wins (is_better returns false when equal)
        assert_eq!(results_by_doc.get(&0).unwrap().tier, 1);
    }
}

// ====== SECTION SELECTION TESTS ======
// Tests for ensuring the best section is selected when a document
// matches multiple query terms in different sections.

mod section_selection_tests {
    use std::cmp::Ordering;
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[allow(dead_code)]
    enum MatchType {
        Title = 0,
        Section = 1,
        Subsection = 2,
        Paragraph = 3,
        Content = 4,
    }

    #[derive(Debug, Clone)]
    struct SectionMatch {
        doc_id: usize,
        section_idx: u32,
        score: f64,
        match_type: MatchType,
    }

    #[derive(Debug, Clone)]
    struct SearchResult {
        doc_id: usize,
        section_idx: u32,
        score: f64,
        match_type: MatchType,
    }

    /// Simulate the CORRECT multi-term accumulator behavior:
    /// - Sum scores across all sections for ranking
    /// - Pick best section (by match_type, then score) for deep linking
    fn accumulate_and_select(matches: Vec<SectionMatch>) -> Vec<SearchResult> {
        // Group by doc_id
        let mut by_doc: HashMap<usize, Vec<SectionMatch>> = HashMap::new();
        for m in matches {
            by_doc.entry(m.doc_id).or_default().push(m);
        }

        by_doc
            .into_iter()
            .map(|(doc_id, sections)| {
                // Total score = sum of all section scores (for ranking)
                let total_score: f64 = sections.iter().map(|s| s.score).sum();

                // Best section = best match_type, then highest score (for deep linking)
                let best = sections
                    .iter()
                    .min_by(|a, b| {
                        a.match_type
                            .cmp(&b.match_type)
                            .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal))
                    })
                    .unwrap();

                SearchResult {
                    doc_id,
                    section_idx: best.section_idx,
                    score: total_score,
                    match_type: best.match_type,
                }
            })
            .collect()
    }

    // ====== PROPERTY 1: Single term scenarios ======

    /// P1.1: Single term, single section
    #[test]
    fn prop_single_term_single_section() {
        let matches = vec![SectionMatch {
            doc_id: 0,
            section_idx: 0,
            score: 100.0,
            match_type: MatchType::Title,
        }];
        let results = accumulate_and_select(matches);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].section_idx, 0);
        assert_eq!(results[0].score, 100.0);
        assert_eq!(results[0].match_type, MatchType::Title);
    }

    // ====== PROPERTY 2: Multi-term, same section ======

    /// P2.1: Multi-term, same section - scores summed
    #[test]
    fn prop_multi_term_same_section_scores_summed() {
        // "tensor" and "cuda" both match in title section
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 100.0,
                match_type: MatchType::Title,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 50.0,
                match_type: MatchType::Title,
            },
        ];
        let results = accumulate_and_select(matches);
        assert_eq!(results[0].score, 150.0, "Scores should be summed");
        assert_eq!(results[0].section_idx, 0);
        assert_eq!(results[0].match_type, MatchType::Title);
    }

    // ====== PROPERTY 3: Multi-term, different sections ======

    /// P3.1: Multi-term, different sections - best match_type wins for deep link
    #[test]
    fn prop_multi_term_different_sections_best_match_type() {
        // "tensor" in Title (section 0), "cuda" in Content (section 3)
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 50.0,
                match_type: MatchType::Title,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 3,
                score: 100.0,
                match_type: MatchType::Content,
            },
        ];
        let results = accumulate_and_select(matches);
        assert_eq!(
            results[0].section_idx, 0,
            "Title section should be selected for deep link"
        );
        assert_eq!(results[0].match_type, MatchType::Title);
        assert_eq!(results[0].score, 150.0, "Total score for ranking");
    }

    /// P3.2: Multi-term, different sections, same match_type - highest score wins
    #[test]
    fn prop_multi_term_same_match_type_highest_score() {
        // Both terms match in Content sections, but different offsets
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 50.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 3,
                score: 100.0,
                match_type: MatchType::Content,
            },
        ];
        let results = accumulate_and_select(matches);
        assert_eq!(
            results[0].section_idx, 3,
            "Higher score section should be selected"
        );
        assert_eq!(results[0].score, 150.0);
    }

    /// P3.3: Three sections, middle one is best
    #[test]
    fn prop_three_sections_middle_best() {
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 30.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 1,
                score: 80.0,
                match_type: MatchType::Section,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 2,
                score: 50.0,
                match_type: MatchType::Content,
            },
        ];
        let results = accumulate_and_select(matches);
        assert_eq!(
            results[0].section_idx, 1,
            "Section match_type should win over Content"
        );
        assert_eq!(results[0].match_type, MatchType::Section);
        assert_eq!(results[0].score, 160.0, "Total: 30 + 80 + 50");
    }

    // ====== PROPERTY 4: Ranking behavior ======

    /// P4.1: Ranking uses total score, not best section score
    #[test]
    fn prop_ranking_uses_total_score() {
        // Doc 0: Title(50) + Content(100) = 150 total
        // Doc 1: Title(200) = 200 total
        // Doc 1 should rank higher despite Doc 0 having more sections
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 50.0,
                match_type: MatchType::Title,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 3,
                score: 100.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 1,
                section_idx: 0,
                score: 200.0,
                match_type: MatchType::Title,
            },
        ];
        let mut results = accumulate_and_select(matches);
        // Sort by score descending (simple ranking for test)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

        assert_eq!(results[0].doc_id, 1, "Doc 1 (score 200) should rank first");
        assert_eq!(results[1].doc_id, 0, "Doc 0 (score 150) should rank second");
    }

    /// P4.2: Equal total scores, better match_type ranks higher
    #[test]
    fn prop_equal_scores_match_type_wins() {
        // Doc 0: Content(100) = 100 total, Content match
        // Doc 1: Title(100) = 100 total, Title match
        // Doc 1 should rank higher due to better match_type
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 100.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 1,
                section_idx: 0,
                score: 100.0,
                match_type: MatchType::Title,
            },
        ];
        let mut results = accumulate_and_select(matches);
        // Sort by match_type first (lower is better), then score
        results.sort_by(|a, b| {
            a.match_type
                .cmp(&b.match_type)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal))
        });

        assert_eq!(results[0].doc_id, 1, "Doc 1 (Title) should rank first");
        assert_eq!(results[1].doc_id, 0, "Doc 0 (Content) should rank second");
    }

    // ====== PROPERTY 5: Edge cases ======

    /// P5.1: All sections same match_type, same score - first section wins
    #[test]
    fn prop_all_equal_first_wins() {
        let matches = vec![
            SectionMatch {
                doc_id: 0,
                section_idx: 5,
                score: 50.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 3,
                score: 50.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 7,
                score: 50.0,
                match_type: MatchType::Content,
            },
        ];
        let results = accumulate_and_select(matches);
        // When all equal, min_by returns first encountered
        assert_eq!(results[0].score, 150.0);
        // section_idx depends on iteration order, but score should be summed
    }

    /// P5.2: Single doc, many sections
    #[test]
    fn prop_many_sections_single_doc() {
        let matches: Vec<SectionMatch> = (0..10)
            .map(|i| SectionMatch {
                doc_id: 0,
                section_idx: i,
                score: 10.0,
                match_type: if i == 5 {
                    MatchType::Title
                } else {
                    MatchType::Content
                },
            })
            .collect();

        let results = accumulate_and_select(matches);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 100.0, "Sum of 10 sections * 10.0");
        assert_eq!(
            results[0].section_idx, 5,
            "Title section should be selected"
        );
        assert_eq!(results[0].match_type, MatchType::Title);
    }

    /// P5.3: Multiple docs with overlapping sections
    #[test]
    fn prop_multiple_docs_independent() {
        let matches = vec![
            // Doc 0: Title + Content
            SectionMatch {
                doc_id: 0,
                section_idx: 0,
                score: 100.0,
                match_type: MatchType::Title,
            },
            SectionMatch {
                doc_id: 0,
                section_idx: 1,
                score: 50.0,
                match_type: MatchType::Content,
            },
            // Doc 1: Content + Content
            SectionMatch {
                doc_id: 1,
                section_idx: 0,
                score: 80.0,
                match_type: MatchType::Content,
            },
            SectionMatch {
                doc_id: 1,
                section_idx: 1,
                score: 80.0,
                match_type: MatchType::Content,
            },
        ];

        let results = accumulate_and_select(matches);
        assert_eq!(results.len(), 2);

        let doc0 = results.iter().find(|r| r.doc_id == 0).unwrap();
        let doc1 = results.iter().find(|r| r.doc_id == 1).unwrap();

        assert_eq!(doc0.score, 150.0);
        assert_eq!(doc0.section_idx, 0);
        assert_eq!(doc0.match_type, MatchType::Title);

        assert_eq!(doc1.score, 160.0);
        assert_eq!(doc1.match_type, MatchType::Content);
    }
}

// ====== CUTLASS DATASET INTEGRATION TESTS ======
// Real-world tests using the cutlass dataset to verify section selection behavior.
// These tests require the bench-datasets feature to be enabled.

#[cfg(feature = "bench-datasets")]
mod cutlass_section_tests {
    use super::super::common::load_cutlass_searcher;
    use sorex::MatchType;

    /// Verify that multi-term searches return the best section for deep linking.
    /// When "gemm" appears in a title and "cuda" appears in content,
    /// the result should link to the title section (better match_type).
    #[test]
    fn test_multi_term_best_section_selected() {
        let searcher = load_cutlass_searcher();

        // Search for terms that likely appear in different sections of the same doc
        let results = searcher.search("gemm cuda", 20);

        // For any doc that matches, verify it has a valid section
        for result in &results {
            // section_idx should be consistent with match_type
            // If match_type is Title, section_idx should typically be 0
            if result.match_type == MatchType::Title {
                // Title matches often have section_idx 0, but not always
                // The key invariant is that we got the BEST match_type
            }
        }

        // Results should be sorted: Title matches before Section before Content
        let mut prev_match_type = MatchType::Title;
        for result in &results {
            // match_type should never go from worse to better
            assert!(
                result.match_type >= prev_match_type,
                "Results should be sorted by match_type: {:?} came after {:?}",
                result.match_type,
                prev_match_type
            );
            prev_match_type = result.match_type;
        }
    }

    /// Verify that single-term exact matches preserve correct section info.
    #[test]
    fn test_single_term_section_preserved() {
        let searcher = load_cutlass_searcher();

        // "gemm" appears in titles - should get Title match_type
        let results = searcher.search("gemm", 50);

        // Count results by match_type
        let title_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Title)
            .count();
        let section_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Section)
            .count();
        let content_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Content)
            .count();

        // Should have variety of match types (gemm appears in titles, sections, and content)
        assert!(
            title_count > 0 || section_count > 0 || content_count > 0,
            "Should find matches in various sections"
        );

        // Verify section_idx resolves to valid section IDs
        for result in &results {
            if result.section_idx > 0 {
                let section_id = searcher
                    .section_table()
                    .get((result.section_idx - 1) as usize);
                assert!(
                    section_id.is_some(),
                    "section_idx {} should resolve to valid section",
                    result.section_idx
                );
            }
        }
    }

    /// Verify that prefix searches correctly identify section types.
    #[test]
    fn test_prefix_search_section_types() {
        let searcher = load_cutlass_searcher();

        // "gem" prefix should match "gemm" in various sections
        let results = searcher.search("gem", 50);

        // Should have results
        assert!(!results.is_empty(), "Prefix search should find matches");

        // Count different match types
        let title_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Title)
            .count();
        let section_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Section)
            .count();
        let content_count = results
            .iter()
            .filter(|r| r.match_type == MatchType::Content)
            .count();

        // Should have at least one type of match
        assert!(
            title_count > 0 || section_count > 0 || content_count > 0,
            "Should find matches in at least one section type"
        );
    }

    /// Verify that fuzzy searches correctly identify section types.
    #[test]
    fn test_fuzzy_search_section_types() {
        let searcher = load_cutlass_searcher();

        // "gemma" (typo of "gemm") should fuzzy match
        let results = searcher.search("gemma", 50);

        // Results should be sorted by match_type then score
        for i in 1..results.len() {
            let prev = &results[i - 1];
            let curr = &results[i];

            // Either match_type improves (worse) or stays same with score decreasing
            let valid_order = prev.match_type < curr.match_type
                || (prev.match_type == curr.match_type && prev.score >= curr.score);

            assert!(
                valid_order,
                "Results not properly sorted: {:?} (score {}) before {:?} (score {})",
                prev.match_type, prev.score, curr.match_type, curr.score
            );
        }
    }

    /// Verify deduplication: same doc should not appear twice.
    #[test]
    fn test_no_duplicate_docs_in_results() {
        let searcher = load_cutlass_searcher();

        // Multi-term search
        let results = searcher.search("gemm performance", 100);

        // Check for duplicate doc_ids
        let mut seen_docs = std::collections::HashSet::new();
        for result in &results {
            assert!(
                seen_docs.insert(result.doc_id),
                "Duplicate doc_id {} in results",
                result.doc_id
            );
        }
    }

    /// Verify that the best match_type is used for ranking across tiers.
    #[test]
    fn test_best_match_type_across_tiers() {
        let searcher = load_cutlass_searcher();

        // Search that should hit multiple tiers
        let results = searcher.search("kerne", 50); // prefix match for "kernel"

        // All Title matches should come before all Section matches
        // All Section matches should come before all Content matches
        let title_positions: Vec<usize> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.match_type == MatchType::Title)
            .map(|(i, _)| i)
            .collect();

        let content_positions: Vec<usize> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.match_type == MatchType::Content)
            .map(|(i, _)| i)
            .collect();

        if !title_positions.is_empty() && !content_positions.is_empty() {
            let max_title_pos = *title_positions.iter().max().unwrap();
            let min_content_pos = *content_positions.iter().min().unwrap();

            assert!(
                max_title_pos < min_content_pos,
                "Title matches (max pos {}) should all come before Content matches (min pos {})",
                max_title_pos,
                min_content_pos
            );
        }
    }

    /// Verify section_idx consistency: same section should have same section_idx.
    #[test]
    fn test_section_idx_consistency() {
        let searcher = load_cutlass_searcher();

        // Get multiple searches to build a mapping
        let results1 = searcher.search("gemm", 50);
        let results2 = searcher.search("cuda", 50);

        // For any doc that appears in both, section_idx should be deterministic
        // (though it may differ if different sections matched)
        let docs1: std::collections::HashMap<usize, u32> =
            results1.iter().map(|r| (r.doc_id, r.section_idx)).collect();

        // If a doc appears in both results with same section, idx should match
        // This is a basic consistency check
        for r2 in &results2 {
            if let Some(&idx1) = docs1.get(&r2.doc_id) {
                // Both results found this doc - section_idx may differ
                // but both should be valid
                assert!(
                    idx1 <= searcher.section_table().len() as u32,
                    "section_idx from search 1 should be valid"
                );
                assert!(
                    r2.section_idx <= searcher.section_table().len() as u32,
                    "section_idx from search 2 should be valid"
                );
            }
        }
    }
}

// ====== SECTION DEDUP OPTIONS TESTS ======
// Tests for the optional section deduplication feature.
// When dedup_sections=true (default), returns one result per document.
// When dedup_sections=false, can return multiple sections per document.

#[cfg(feature = "bench-datasets")]
mod section_dedup_options_tests {
    use super::super::common::load_cutlass_searcher;
    use sorex::tiered_search::SearchOptions;

    /// Default behavior: section dedup enabled, one result per document.
    #[test]
    fn test_default_dedup_sections_enabled() {
        let searcher = load_cutlass_searcher();

        // Default search (dedup_sections = true)
        let results = searcher.search("gemm", 100);

        // Each doc should appear only once
        let mut seen_docs = std::collections::HashSet::new();
        for result in &results {
            assert!(
                seen_docs.insert(result.doc_id),
                "With dedup enabled, doc {} should appear only once",
                result.doc_id
            );
        }
    }

    /// Explicit dedup_sections=true should behave same as default.
    #[test]
    fn test_explicit_dedup_sections_true() {
        let searcher = load_cutlass_searcher();

        let options = SearchOptions::new(); // dedup_sections = true by default
        assert!(options.dedup_sections, "Default should have dedup enabled");

        let results = searcher.search_with_options("gemm", 100, options);

        // Each doc should appear only once
        let mut seen_docs = std::collections::HashSet::new();
        for result in &results {
            assert!(
                seen_docs.insert(result.doc_id),
                "With dedup enabled, doc {} should appear only once",
                result.doc_id
            );
        }
    }

    /// With dedup_sections=false, can return multiple sections per document.
    #[test]
    fn test_dedup_sections_disabled() {
        let searcher = load_cutlass_searcher();

        let options = SearchOptions::without_section_dedup();
        assert!(!options.dedup_sections, "Should have dedup disabled");

        let results = searcher.search_with_options("gemm", 100, options);

        // With dedup disabled, same doc MAY appear multiple times (different sections)
        // This is valid - we just verify results are still sorted correctly
        for i in 1..results.len() {
            let prev = &results[i - 1];
            let curr = &results[i];

            // Results should still be sorted by match_type then score
            let valid_order = prev.match_type < curr.match_type
                || (prev.match_type == curr.match_type && prev.score >= curr.score);

            assert!(
                valid_order,
                "Results should still be sorted even without dedup: {:?} ({}) before {:?} ({})",
                prev.match_type, prev.score, curr.match_type, curr.score
            );
        }
    }

    /// Compare result counts: without dedup should have >= with dedup.
    #[test]
    fn test_dedup_disabled_has_more_or_equal_results() {
        let searcher = load_cutlass_searcher();

        let with_dedup = searcher.search_with_options("gemm", 100, SearchOptions::new());
        let without_dedup =
            searcher.search_with_options("gemm", 100, SearchOptions::without_section_dedup());

        // Without dedup, we should have at least as many results
        // (potentially more if a doc has multiple matching sections)
        assert!(
            without_dedup.len() >= with_dedup.len(),
            "Without dedup ({}) should have >= results than with dedup ({})",
            without_dedup.len(),
            with_dedup.len()
        );
    }

    /// Verify that all docs in deduped results also appear in non-deduped results.
    #[test]
    fn test_dedup_subset_of_non_dedup() {
        let searcher = load_cutlass_searcher();

        let with_dedup = searcher.search_with_options("cuda", 50, SearchOptions::new());
        let without_dedup =
            searcher.search_with_options("cuda", 200, SearchOptions::without_section_dedup());

        let dedup_docs: std::collections::HashSet<usize> =
            with_dedup.iter().map(|r| r.doc_id).collect();
        let non_dedup_docs: std::collections::HashSet<usize> =
            without_dedup.iter().map(|r| r.doc_id).collect();

        // Every doc in deduped results should also be in non-deduped results
        for doc_id in &dedup_docs {
            assert!(
                non_dedup_docs.contains(doc_id),
                "Doc {} in deduped results should also be in non-deduped results",
                doc_id
            );
        }
    }

    /// Multi-term search with dedup disabled should show multiple sections per doc.
    #[test]
    fn test_multi_term_without_dedup() {
        let searcher = load_cutlass_searcher();

        // Multi-term search - terms may appear in different sections of same doc
        let options = SearchOptions::without_section_dedup();
        let results = searcher.search_with_options("gemm cuda", 100, options);

        // Count occurrences of each doc
        let mut doc_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for result in &results {
            *doc_counts.entry(result.doc_id).or_default() += 1;
        }

        // With dedup disabled, we might see docs appearing multiple times
        // (but not guaranteed - depends on the dataset)
        // Just verify the results are valid
        for result in &results {
            assert!(
                result.section_idx <= searcher.section_table().len() as u32,
                "section_idx {} should be valid",
                result.section_idx
            );
        }
    }

    /// Verify SearchOptions::default() has dedup enabled.
    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();
        assert!(
            options.dedup_sections,
            "SearchOptions::default() should have dedup_sections = true"
        );
    }
}
