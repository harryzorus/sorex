//! SA-IS: Suffix Array by Induced Sorting
//!
//! Linear-time O(n) suffix array construction algorithm.
//!
//! # Algorithm Overview
//!
//! ```text
//! Input: "banana"
//!
//! Step 1: Append sentinel (0) and classify suffixes
//! ┌───┬───┬───┬───┬───┬───┬───┐
//! │ b │ a │ n │ a │ n │ a │ $ │   ($ = sentinel, value 0)
//! ├───┼───┼───┼───┼───┼───┼───┤
//! │ L │ S │ L │ S │ L │ S │ S │   (S = smaller than next, L = larger)
//! └───┴───┴───┴───┴───┴───┴───┘
//!
//! Step 2: Find LMS (Leftmost S-type) suffixes
//!         LMS = S-type preceded by L-type
//!         Positions: 1, 3, 5, 6
//!
//! Step 3: Induced sorting
//!         - Place LMS suffixes at bucket tails
//!         - Induce L-type positions (left-to-right)
//!         - Induce S-type positions (right-to-left)
//!
//! Step 4: If LMS substrings not unique, recurse on reduced problem
//!
//! Step 5: Use sorted LMS order to induce final suffix array
//!
//! Output: [6, 5, 3, 1, 0, 4, 2]
//! ```
//!
//! # Complexity
//!
//! - Time: O(n)
//! - Space: O(n)
//!
//! # References
//!
//! - Nong, Zhang, Chan (2009): "Linear Suffix Array Construction by Almost Pure Induced-Sorting"
//! - <https://doi.org/10.1109/DCC.2009.42>

use crate::types::VocabSuffixEntry;

/// Suffix type classification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SuffixType {
    /// S-type: suffix is lexicographically smaller than the next suffix
    S,
    /// L-type: suffix is lexicographically larger than the next suffix
    L,
}

/// Sentinel value (must be smaller than all input characters).
const SENTINEL: u8 = 0;

/// Build suffix array using SA-IS algorithm in O(n) time.
///
/// # Arguments
/// * `text` - Input text as byte slice
///
/// # Returns
/// Suffix array: `sa[i]` = starting position of the i-th smallest suffix
pub fn sais(text: &[u8]) -> Vec<usize> {
    if text.is_empty() {
        return Vec::new();
    }

    // Append sentinel to ensure proper termination
    let mut text_with_sentinel = text.to_vec();
    text_with_sentinel.push(SENTINEL);

    let sa = sais_inner(&text_with_sentinel, 256);

    // Remove the sentinel position from the result
    // The sentinel is always at position len-1 and always sorts first
    sa.into_iter().filter(|&pos| pos < text.len()).collect()
}

/// Core SA-IS implementation for u8 alphabet.
fn sais_inner(text: &[u8], alphabet_size: usize) -> Vec<usize> {
    let n = text.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }

    // Step 1: Classify suffixes
    let types = classify_suffixes_u8(text);

    // Step 2: Find LMS positions
    let lms_positions: Vec<usize> = (1..n).filter(|&i| is_lms(&types, i)).collect();

    // Step 3: Bucket sort setup
    let bucket_sizes = compute_bucket_sizes_u8(text, alphabet_size);

    // Step 4: First induced sort to determine LMS order
    let mut sa = vec![usize::MAX; n];

    // Place LMS suffixes
    let mut tails = compute_bucket_tails(&bucket_sizes);
    for &pos in lms_positions.iter().rev() {
        let c = text[pos] as usize;
        tails[c] -= 1;
        sa[tails[c]] = pos;
    }

    // Induce L-type
    let mut heads = compute_bucket_heads(&bucket_sizes);
    for i in 0..n {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::L {
            let c = text[j] as usize;
            sa[heads[c]] = j;
            heads[c] += 1;
        }
    }

    // Induce S-type
    let mut tails = compute_bucket_tails(&bucket_sizes);
    for i in (0..n).rev() {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::S {
            let c = text[j] as usize;
            tails[c] -= 1;
            sa[tails[c]] = j;
        }
    }

    // Step 5: Name LMS substrings
    let mut name = 0usize;
    let mut prev_pos: Option<usize> = None;
    let mut lms_names = vec![0usize; n];

    for i in 0..n {
        let pos = sa[i];
        if !is_lms(&types, pos) {
            continue;
        }

        // Check if different from previous LMS substring
        if let Some(prev) = prev_pos {
            if !lms_substrings_equal_u8(text, &types, prev, pos) {
                name += 1;
            }
        }

        lms_names[pos] = name;
        prev_pos = Some(pos);
    }

    let unique_count = name + 1;

    // Step 6: Build reduced string from LMS names (in text order)
    let reduced: Vec<usize> = lms_positions.iter().map(|&pos| lms_names[pos]).collect();

    // Step 7: Recursively sort if not all unique
    let sorted_lms_indices = if unique_count < lms_positions.len() {
        sais_recursive(&reduced, unique_count)
    } else {
        // All unique: the names themselves give the order
        let mut order: Vec<usize> = (0..reduced.len()).collect();
        order.sort_by_key(|&i| reduced[i]);
        order
    };

    // Step 8: Final induced sort with correctly ordered LMS suffixes
    let sorted_lms: Vec<usize> = sorted_lms_indices
        .iter()
        .map(|&i| lms_positions[i])
        .collect();

    sa.fill(usize::MAX);

    // Place LMS in correct order
    let mut tails = compute_bucket_tails(&bucket_sizes);
    for &pos in sorted_lms.iter().rev() {
        let c = text[pos] as usize;
        tails[c] -= 1;
        sa[tails[c]] = pos;
    }

    // Induce L-type
    let mut heads = compute_bucket_heads(&bucket_sizes);
    for i in 0..n {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::L {
            let c = text[j] as usize;
            sa[heads[c]] = j;
            heads[c] += 1;
        }
    }

    // Induce S-type
    let mut tails = compute_bucket_tails(&bucket_sizes);
    for i in (0..n).rev() {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::S {
            let c = text[j] as usize;
            tails[c] -= 1;
            sa[tails[c]] = j;
        }
    }

    sa
}

/// Recursive SA-IS for integer alphabet.
fn sais_recursive(text: &[usize], alphabet_size: usize) -> Vec<usize> {
    let n = text.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }
    if n == 2 {
        return if text[0] <= text[1] {
            vec![0, 1]
        } else {
            vec![1, 0]
        };
    }

    // Classify suffixes
    let types = classify_suffixes_usize(text);

    // Find LMS positions
    let lms_positions: Vec<usize> = (1..n).filter(|&i| is_lms(&types, i)).collect();

    if lms_positions.is_empty() {
        // No LMS means all L-type or all S-type
        // Just sort directly
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| text[a..].cmp(&text[b..]));
        return sa;
    }

    // Bucket sort setup
    let bucket_sizes = compute_bucket_sizes_usize(text, alphabet_size);

    // First induced sort
    let mut sa = vec![usize::MAX; n];

    let mut tails = compute_bucket_tails(&bucket_sizes);
    for &pos in lms_positions.iter().rev() {
        let c = text[pos];
        tails[c] -= 1;
        sa[tails[c]] = pos;
    }

    let mut heads = compute_bucket_heads(&bucket_sizes);
    for i in 0..n {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::L {
            let c = text[j];
            sa[heads[c]] = j;
            heads[c] += 1;
        }
    }

    let mut tails = compute_bucket_tails(&bucket_sizes);
    for i in (0..n).rev() {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::S {
            let c = text[j];
            tails[c] -= 1;
            sa[tails[c]] = j;
        }
    }

    // Name LMS substrings
    let mut name = 0usize;
    let mut prev_pos: Option<usize> = None;
    let mut lms_names = vec![0usize; n];

    for i in 0..n {
        let pos = sa[i];
        if !is_lms(&types, pos) {
            continue;
        }

        if let Some(prev) = prev_pos {
            if !lms_substrings_equal_usize(text, &types, prev, pos) {
                name += 1;
            }
        }

        lms_names[pos] = name;
        prev_pos = Some(pos);
    }

    let unique_count = name + 1;
    let reduced: Vec<usize> = lms_positions.iter().map(|&pos| lms_names[pos]).collect();

    // Recurse
    let sorted_lms_indices = if unique_count < lms_positions.len() {
        sais_recursive(&reduced, unique_count)
    } else {
        let mut order: Vec<usize> = (0..reduced.len()).collect();
        order.sort_by_key(|&i| reduced[i]);
        order
    };

    let sorted_lms: Vec<usize> = sorted_lms_indices
        .iter()
        .map(|&i| lms_positions[i])
        .collect();

    // Final induced sort
    sa.fill(usize::MAX);

    let mut tails = compute_bucket_tails(&bucket_sizes);
    for &pos in sorted_lms.iter().rev() {
        let c = text[pos];
        tails[c] -= 1;
        sa[tails[c]] = pos;
    }

    let mut heads = compute_bucket_heads(&bucket_sizes);
    for i in 0..n {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::L {
            let c = text[j];
            sa[heads[c]] = j;
            heads[c] += 1;
        }
    }

    let mut tails = compute_bucket_tails(&bucket_sizes);
    for i in (0..n).rev() {
        if sa[i] == usize::MAX || sa[i] == 0 {
            continue;
        }
        let j = sa[i] - 1;
        if types[j] == SuffixType::S {
            let c = text[j];
            tails[c] -= 1;
            sa[tails[c]] = j;
        }
    }

    sa
}

/// Classify each suffix as S-type or L-type.
fn classify_suffixes_u8(text: &[u8]) -> Vec<SuffixType> {
    let n = text.len();
    let mut types = vec![SuffixType::S; n];

    // Last position is always S-type (sentinel)
    types[n - 1] = SuffixType::S;

    for i in (0..n - 1).rev() {
        types[i] = if text[i] > text[i + 1] {
            SuffixType::L
        } else if text[i] < text[i + 1] {
            SuffixType::S
        } else {
            types[i + 1]
        };
    }

    types
}

/// Classify suffixes for usize alphabet.
fn classify_suffixes_usize(text: &[usize]) -> Vec<SuffixType> {
    let n = text.len();
    let mut types = vec![SuffixType::S; n];

    types[n - 1] = SuffixType::S;

    for i in (0..n - 1).rev() {
        types[i] = if text[i] > text[i + 1] {
            SuffixType::L
        } else if text[i] < text[i + 1] {
            SuffixType::S
        } else {
            types[i + 1]
        };
    }

    types
}

/// Check if position i is an LMS position.
#[inline]
fn is_lms(types: &[SuffixType], i: usize) -> bool {
    i > 0 && types[i] == SuffixType::S && types[i - 1] == SuffixType::L
}

/// Compute bucket sizes for u8 alphabet.
fn compute_bucket_sizes_u8(text: &[u8], alphabet_size: usize) -> Vec<usize> {
    let mut sizes = vec![0; alphabet_size];
    for &c in text {
        sizes[c as usize] += 1;
    }
    sizes
}

/// Compute bucket sizes for usize alphabet.
fn compute_bucket_sizes_usize(text: &[usize], alphabet_size: usize) -> Vec<usize> {
    let mut sizes = vec![0; alphabet_size];
    for &c in text {
        sizes[c] += 1;
    }
    sizes
}

/// Compute bucket head positions.
fn compute_bucket_heads(sizes: &[usize]) -> Vec<usize> {
    let mut heads = vec![0; sizes.len()];
    let mut sum = 0;
    for (i, &size) in sizes.iter().enumerate() {
        heads[i] = sum;
        sum += size;
    }
    heads
}

/// Compute bucket tail positions.
fn compute_bucket_tails(sizes: &[usize]) -> Vec<usize> {
    let mut tails = vec![0; sizes.len()];
    let mut sum = 0;
    for (i, &size) in sizes.iter().enumerate() {
        sum += size;
        tails[i] = sum;
    }
    tails
}

/// Compare two LMS substrings for equality (u8 version).
fn lms_substrings_equal_u8(text: &[u8], types: &[SuffixType], i: usize, j: usize) -> bool {
    if i == j {
        return true;
    }

    let n = text.len();
    let mut k = 0;

    loop {
        let pi = i + k;
        let pj = j + k;

        // Check bounds
        if pi >= n || pj >= n {
            return pi >= n && pj >= n;
        }

        // Check character equality
        if text[pi] != text[pj] {
            return false;
        }

        // Check type equality
        if types[pi] != types[pj] {
            return false;
        }

        // After first character, check if both reached next LMS
        if k > 0 {
            let lms_i = is_lms(types, pi);
            let lms_j = is_lms(types, pj);
            if lms_i && lms_j {
                return true;
            }
            if lms_i != lms_j {
                return false;
            }
        }

        k += 1;
    }
}

/// Compare two LMS substrings for equality (usize version).
fn lms_substrings_equal_usize(text: &[usize], types: &[SuffixType], i: usize, j: usize) -> bool {
    if i == j {
        return true;
    }

    let n = text.len();
    let mut k = 0;

    loop {
        let pi = i + k;
        let pj = j + k;

        if pi >= n || pj >= n {
            return pi >= n && pj >= n;
        }

        if text[pi] != text[pj] {
            return false;
        }

        if types[pi] != types[pj] {
            return false;
        }

        if k > 0 {
            let lms_i = is_lms(types, pi);
            let lms_j = is_lms(types, pj);
            if lms_i && lms_j {
                return true;
            }
            if lms_i != lms_j {
                return false;
            }
        }

        k += 1;
    }
}

// =============================================================================
// PUBLIC API FOR VOCABULARY SUFFIX ARRAY
// =============================================================================

/// Build vocabulary suffix array using SA-IS in O(n) time.
///
/// Creates suffix array entries for all suffixes of all vocabulary terms.
/// This enables O(log k) binary search for substring matching.
pub fn build_vocab_suffix_array_sais(vocabulary: &[String]) -> Vec<VocabSuffixEntry> {
    if vocabulary.is_empty() {
        return Vec::new();
    }

    // Concatenate terms with unique separators between them
    // Separator must be < 'a' (smallest letter) so suffixes don't cross boundaries incorrectly
    let mut concat: Vec<u8> = Vec::new();
    let mut term_info: Vec<(usize, usize)> = Vec::new(); // (start_pos, term_idx)

    for (term_idx, term) in vocabulary.iter().enumerate() {
        if term.is_empty() {
            continue;
        }
        term_info.push((concat.len(), term_idx));
        concat.extend(term.as_bytes());
        // Use separator byte 1 (between sentinel 0 and printable chars)
        concat.push(1);
    }

    if concat.is_empty() {
        return Vec::new();
    }

    // Build suffix array
    let sa = sais(&concat);

    // Convert SA positions back to (term_idx, offset) pairs
    let mut entries: Vec<VocabSuffixEntry> = Vec::new();

    for &pos in &sa {
        // Binary search to find which term this position belongs to
        let term_result = term_info.binary_search_by(|&(start, _)| start.cmp(&pos));

        let (term_start, term_idx) = match term_result {
            Ok(i) => term_info[i],
            Err(0) => continue, // Before first term (shouldn't happen)
            Err(i) => term_info[i - 1],
        };

        let term_len = vocabulary[term_idx].len();
        let offset = pos - term_start;

        // Only include if within the term (not at separator)
        if offset < term_len {
            entries.push(VocabSuffixEntry { term_idx, offset });
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sais_simple() {
        let text = b"banana";
        let sa = sais(text);

        // Verify completeness
        assert_eq!(sa.len(), text.len());

        // Verify sortedness
        for i in 1..sa.len() {
            let suffix_a = &text[sa[i - 1]..];
            let suffix_b = &text[sa[i]..];
            assert!(
                suffix_a <= suffix_b,
                "Not sorted at {}: {:?} > {:?}",
                i,
                std::str::from_utf8(suffix_a),
                std::str::from_utf8(suffix_b)
            );
        }

        // Verify all positions present
        let mut positions: Vec<_> = sa.clone();
        positions.sort();
        assert_eq!(positions, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_sais_single_char() {
        let text = b"a";
        let sa = sais(text);
        assert_eq!(sa, vec![0]);
    }

    #[test]
    fn test_sais_repeated() {
        let text = b"aaa";
        let sa = sais(text);

        // Verify sorted (all suffixes start with 'a')
        // a < aa < aaa
        assert_eq!(sa.len(), 3);
        for i in 1..sa.len() {
            let suffix_a = &text[sa[i - 1]..];
            let suffix_b = &text[sa[i]..];
            assert!(suffix_a <= suffix_b);
        }
    }

    #[test]
    fn test_sais_mississippi() {
        let text = b"mississippi";
        let sa = sais(text);

        // Verify completeness
        assert_eq!(sa.len(), text.len());

        // Verify sortedness
        for i in 1..sa.len() {
            let suffix_a = &text[sa[i - 1]..];
            let suffix_b = &text[sa[i]..];
            assert!(
                suffix_a <= suffix_b,
                "Not sorted at {}: {:?} > {:?}",
                i,
                std::str::from_utf8(suffix_a),
                std::str::from_utf8(suffix_b)
            );
        }

        // Verify all positions present
        let mut positions: Vec<_> = sa.clone();
        positions.sort();
        let expected: Vec<_> = (0..text.len()).collect();
        assert_eq!(positions, expected);
    }

    #[test]
    fn test_classify_suffixes() {
        // "banana" + sentinel
        let text = b"banana\x00";
        let types = classify_suffixes_u8(text);

        // Expected: b(L) a(S) n(L) a(S) n(L) a(L) $(S)
        // L-type = suffix is larger than next, S-type = suffix is smaller
        assert_eq!(types[0], SuffixType::L); // b > a
        assert_eq!(types[1], SuffixType::S); // a < n
        assert_eq!(types[2], SuffixType::L); // n > a
        assert_eq!(types[3], SuffixType::S); // a < n
        assert_eq!(types[4], SuffixType::L); // n > a
        assert_eq!(types[5], SuffixType::L); // a > $ (sentinel), so L-type
        assert_eq!(types[6], SuffixType::S); // $ is always S (last position)
    }

    #[test]
    fn test_vocab_suffix_array_simple() {
        let vocabulary = vec!["rust".to_string(), "typescript".to_string()];
        let sa = build_vocab_suffix_array_sais(&vocabulary);

        // Verify all entries are valid
        for entry in &sa {
            assert!(entry.term_idx < vocabulary.len());
            assert!(entry.offset < vocabulary[entry.term_idx].len());
        }

        // Verify sortedness
        for i in 1..sa.len() {
            let suffix_a = &vocabulary[sa[i - 1].term_idx][sa[i - 1].offset..];
            let suffix_b = &vocabulary[sa[i].term_idx][sa[i].offset..];
            assert!(
                suffix_a <= suffix_b,
                "Not sorted: {:?} > {:?}",
                suffix_a,
                suffix_b
            );
        }
    }

    #[test]
    fn test_vocab_suffix_array_finds_script() {
        let vocabulary = vec!["javascript".to_string(), "typescript".to_string()];
        let sa = build_vocab_suffix_array_sais(&vocabulary);

        // Find entries where suffix starts with "script"
        let query = "script";
        let found: Vec<_> = sa
            .iter()
            .filter(|e| vocabulary[e.term_idx][e.offset..].starts_with(query))
            .collect();

        // Should find entries for both javascript and typescript
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_vocab_suffix_array_empty() {
        let vocabulary: Vec<String> = vec![];
        let sa = build_vocab_suffix_array_sais(&vocabulary);
        assert!(sa.is_empty());
    }

    #[test]
    fn test_sais_correctness_random() {
        let test_cases = [
            "abracadabra",
            "aaaaaa",
            "zyxwvutsrqponmlkjihgfedcba",
            "abcdefghijklmnopqrstuvwxyz",
            "the quick brown fox jumps over the lazy dog",
        ];

        for text_str in test_cases {
            let text = text_str.as_bytes();
            let sa = sais(text);

            // Verify completeness
            assert_eq!(sa.len(), text.len(), "Wrong length for {}", text_str);

            // Verify all positions present
            let mut positions: Vec<_> = sa.clone();
            positions.sort();
            let expected: Vec<_> = (0..text.len()).collect();
            assert_eq!(positions, expected, "Missing positions for {}", text_str);

            // Verify sortedness
            for i in 1..sa.len() {
                let suffix_a = &text[sa[i - 1]..];
                let suffix_b = &text[sa[i]..];
                assert!(
                    suffix_a <= suffix_b,
                    "Not sorted at {} for {}: {:?} > {:?}",
                    i,
                    text_str,
                    std::str::from_utf8(suffix_a),
                    std::str::from_utf8(suffix_b)
                );
            }
        }
    }

    #[test]
    fn test_empty_and_two_char() {
        assert!(sais(b"").is_empty());

        let sa = sais(b"ab");
        assert_eq!(sa.len(), 2);
        assert!(b"ab"[sa[0]..] < b"ab"[sa[1]..]);

        let sa = sais(b"ba");
        assert_eq!(sa.len(), 2);
        assert!(b"ba"[sa[0]..] < b"ba"[sa[1]..]);
    }
}
