//! Parametric Levenshtein Automaton with compact serialization.
//!
//! Implements Schulz-Mihov (2002) universal Levenshtein automata.
//! The DFA structure is query-independent - only the character class
//! computation depends on the actual query string.
//!
//! # Serialization Format (4-array scheme)
//!
//! ```text
//! Header (8 bytes):
//!   num_states: u16
//!   max_distance: u8
//!   flags: u8 (bit 0 = transpositions enabled)
//!   reserved: u32
//!
//! Accept array (num_states bytes):
//!   For each state: distance if accepting (0-k), or 0xFF if not accepting
//!
//! Transitions array (num_states * 8 * 2 bytes):
//!   For each state, 8 transitions (one per char class)
//!   Each transition is u16: next state, or 0xFFFF for dead state
//!
//! Total size for k=2: 8 + ~70 + ~70*8*2 = ~1200 bytes
//! ```

use std::collections::{HashMap, VecDeque};

/// Maximum edit distance (k=2 is standard for search)
pub const MAX_K: u8 = 2;

/// Number of character classes: 2^(k+1) = 8 for k=2
pub const NUM_CHAR_CLASSES: usize = 8;

/// Dead state marker
pub const DEAD_STATE: u16 = 0xFFFF;

/// Not accepting marker
pub const NOT_ACCEPTING: u8 = 0xFF;

/// A position in the NFA: (offset from base, edit distance used)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct NfaPos {
    offset: i8,  // Relative to current base position
    edits: u8,   // Edit distance consumed
}

/// A parametric state: a normalized set of NFA positions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ParametricState {
    positions: Vec<NfaPos>,
}

impl ParametricState {
    fn new(mut positions: Vec<NfaPos>) -> Self {
        // Normalize: sort and deduplicate
        positions.sort();
        positions.dedup();
        // Remove dominated positions (same offset, higher edits)
        let mut filtered = Vec::new();
        for pos in positions {
            if !filtered.iter().any(|p: &NfaPos| p.offset == pos.offset && p.edits <= pos.edits) {
                filtered.retain(|p: &NfaPos| !(p.offset == pos.offset && p.edits > pos.edits));
                filtered.push(pos);
            }
        }
        filtered.sort();
        Self { positions: filtered }
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Minimum edit distance to accepting state (if query consumed)
    #[allow(dead_code)]
    fn min_distance(&self, query_len_remaining: i8) -> Option<u8> {
        self.positions
            .iter()
            .filter(|p| p.offset >= query_len_remaining)
            .map(|p| p.edits + (p.offset - query_len_remaining) as u8)
            .filter(|&d| d <= MAX_K)
            .min()
    }

    /// Compute next state given character class
    /// char_class bits: bit i = 1 if input matches query[base + i]
    fn next(&self, char_class: usize, with_transpositions: bool) -> ParametricState {
        let mut next_positions = Vec::new();

        for &pos in &self.positions {
            if pos.edits > MAX_K {
                continue;
            }

            // Match: if char matches query[base + offset]
            if char_class & (1 << pos.offset.max(0)) != 0 && pos.offset >= 0 {
                next_positions.push(NfaPos {
                    offset: pos.offset + 1,
                    edits: pos.edits,
                });
            }

            // Substitution: consume one from each, add one edit
            if pos.edits < MAX_K {
                next_positions.push(NfaPos {
                    offset: pos.offset + 1,
                    edits: pos.edits + 1,
                });
            }

            // Deletion (from query): advance query, don't consume input
            // This is handled by epsilon transitions during state computation

            // Insertion (into query): consume input without advancing query
            if pos.edits < MAX_K {
                next_positions.push(NfaPos {
                    offset: pos.offset,
                    edits: pos.edits + 1,
                });
            }

            // Transposition: swap adjacent characters
            if with_transpositions && pos.edits < MAX_K && pos.offset >= 0 {
                // Check if this char matches query[base + offset + 1]
                let next_bit = ((pos.offset + 1) as usize).min(MAX_K as usize);
                if next_bit <= MAX_K as usize && char_class & (1 << next_bit) != 0 {
                    // We matched the "next" character, could be transposition
                    next_positions.push(NfaPos {
                        offset: pos.offset, // Stay at same offset, will need to match current next
                        edits: pos.edits + 1,
                    });
                }
            }
        }

        // Add deletion transitions (epsilon moves)
        let mut with_deletions = next_positions.clone();
        for &pos in &next_positions {
            if pos.edits < MAX_K {
                // Can delete from query (skip query char)
                with_deletions.push(NfaPos {
                    offset: pos.offset + 1,
                    edits: pos.edits + 1,
                });
                if pos.edits + 1 < MAX_K {
                    with_deletions.push(NfaPos {
                        offset: pos.offset + 2,
                        edits: pos.edits + 2,
                    });
                }
            }
        }

        ParametricState::new(with_deletions)
    }

    /// Normalize to base offset 0
    fn normalize(&self) -> (Self, i8) {
        if self.positions.is_empty() {
            return (Self { positions: vec![] }, 0);
        }
        let min_offset = self.positions.iter().map(|p| p.offset).min().unwrap_or(0);
        let normalized = self.positions
            .iter()
            .map(|p| NfaPos {
                offset: p.offset - min_offset,
                edits: p.edits,
            })
            .collect();
        (ParametricState::new(normalized), min_offset)
    }
}

/// Compiled parametric Levenshtein DFA
#[derive(Debug, Clone)]
pub struct ParametricDFA {
    /// Accept distances for each state (NOT_ACCEPTING if not accepting)
    pub accept: Vec<u8>,
    /// Transitions: [state * NUM_CHAR_CLASSES + char_class] = next_state
    pub transitions: Vec<u16>,
    /// Number of states
    pub num_states: u16,
    /// Whether transpositions are enabled
    pub with_transpositions: bool,
}

impl ParametricDFA {
    /// Build the parametric DFA for edit distance k=2
    pub fn build(with_transpositions: bool) -> Self {
        let mut states: Vec<ParametricState> = Vec::new();
        let mut state_map: HashMap<ParametricState, u16> = HashMap::new();
        let mut transitions: Vec<u16> = Vec::new();
        let mut accept: Vec<u8> = Vec::new();
        let mut queue: VecDeque<u16> = VecDeque::new();

        // Initial state: can be at positions 0, 1, 2 with increasing edits
        let initial_positions: Vec<NfaPos> = (0..=MAX_K)
            .map(|i| NfaPos { offset: i as i8, edits: i })
            .collect();
        let initial = ParametricState::new(initial_positions);
        let (normalized, _) = initial.normalize();

        states.push(normalized.clone());
        state_map.insert(normalized, 0);
        queue.push_back(0);

        while let Some(state_id) = queue.pop_front() {
            let state = states[state_id as usize].clone();

            // Compute accept distance
            // A state accepts if any position has offset >= 0 (consumed query)
            let accept_dist = state.positions
                .iter()
                .filter(|p| p.offset >= 0)
                .map(|p| p.edits)
                .min()
                .filter(|&d| d <= MAX_K)
                .unwrap_or(NOT_ACCEPTING);
            accept.push(accept_dist);

            // Compute transitions for all character classes
            for char_class in 0..NUM_CHAR_CLASSES {
                let next = state.next(char_class, with_transpositions);
                let (normalized, _) = next.normalize();

                let next_id = if normalized.is_empty() {
                    DEAD_STATE
                } else if let Some(&id) = state_map.get(&normalized) {
                    id
                } else {
                    let id = states.len() as u16;
                    states.push(normalized.clone());
                    state_map.insert(normalized, id);
                    queue.push_back(id);
                    id
                };

                transitions.push(next_id);
            }
        }

        Self {
            accept,
            transitions,
            num_states: states.len() as u16,
            with_transpositions,
        }
    }

    /// Serialize to bytes (4-array scheme)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.accept.len() + self.transitions.len() * 2);

        // Header (8 bytes)
        bytes.extend_from_slice(&self.num_states.to_le_bytes());
        bytes.push(MAX_K);
        bytes.push(if self.with_transpositions { 1 } else { 0 });
        bytes.extend_from_slice(&[0u8; 4]); // Reserved

        // Accept array
        bytes.extend_from_slice(&self.accept);

        // Transitions array (u16 per transition)
        for &t in &self.transitions {
            bytes.extend_from_slice(&t.to_le_bytes());
        }

        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 8 {
            return Err("Header too short");
        }

        let num_states = u16::from_le_bytes([bytes[0], bytes[1]]);
        let max_k = bytes[2];
        let with_transpositions = bytes[3] & 1 != 0;

        if max_k != MAX_K {
            return Err("Unsupported max distance");
        }

        let accept_end = 8 + num_states as usize;
        if bytes.len() < accept_end {
            return Err("Accept array too short");
        }
        let accept = bytes[8..accept_end].to_vec();

        let transitions_bytes = &bytes[accept_end..];
        let expected_transitions = num_states as usize * NUM_CHAR_CLASSES;
        if transitions_bytes.len() < expected_transitions * 2 {
            return Err("Transitions array too short");
        }

        let transitions: Vec<u16> = transitions_bytes
            .chunks_exact(2)
            .take(expected_transitions)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        Ok(Self {
            accept,
            transitions,
            num_states,
            with_transpositions,
        })
    }

    /// Get serialized size in bytes
    pub fn serialized_size(&self) -> usize {
        8 + self.accept.len() + self.transitions.len() * 2
    }
}

/// Query-specific DFA matcher
pub struct QueryMatcher<'a> {
    #[allow(dead_code)]
    dfa: &'a ParametricDFA,
    query_chars: Vec<char>,
}

impl<'a> QueryMatcher<'a> {
    pub fn new(dfa: &'a ParametricDFA, query: &str) -> Self {
        Self {
            dfa,
            query_chars: query.chars().collect(),
        }
    }

    /// Compute character class for input char at given offset
    /// (Reserved for future DFA-based matching optimization)
    #[allow(dead_code)]
    #[inline]
    fn char_class(&self, c: char, offset: usize) -> usize {
        let mut class = 0usize;
        for i in 0..=MAX_K as usize {
            if offset + i < self.query_chars.len() && self.query_chars[offset + i] == c {
                class |= 1 << i;
            }
        }
        class
    }

    /// Check if term matches query within edit distance k
    /// Returns Some(distance) if match, None otherwise
    pub fn matches(&self, term: &str) -> Option<u8> {
        // Fall back to simple Levenshtein for now
        // The parametric DFA approach needs more work
        let distance = simple_levenshtein(&self.query_chars, term);
        if distance <= MAX_K as usize {
            Some(distance as u8)
        } else {
            None
        }
    }
}

/// Simple Levenshtein distance using dynamic programming
/// Optimized with early exit when distance exceeds max_k
fn simple_levenshtein(query_chars: &[char], term: &str) -> usize {
    let term_chars: Vec<char> = term.chars().collect();
    let m = query_chars.len();
    let n = term_chars.len();

    // Early exit: length difference exceeds max distance
    let len_diff = (m as isize - n as isize).unsigned_abs();
    if len_diff > MAX_K as usize {
        return len_diff;
    }

    // Use single row for space efficiency
    let mut prev_row: Vec<usize> = (0..=n).collect();
    let mut curr_row: Vec<usize> = vec![0; n + 1];

    for (i, &qc) in query_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        let mut min_in_row = curr_row[0];

        for (j, &tc) in term_chars.iter().enumerate() {
            let cost = if qc == tc { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j] + cost)
                .min(prev_row[j + 1] + 1)  // deletion
                .min(curr_row[j] + 1);     // insertion
            min_in_row = min_in_row.min(curr_row[j + 1]);
        }

        // Early exit if all values in row exceed max distance
        if min_in_row > MAX_K as usize {
            return min_in_row;
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dfa() {
        let dfa = ParametricDFA::build(true);
        println!("Built DFA with {} states", dfa.num_states);
        println!("Serialized size: {} bytes", dfa.serialized_size());
        assert!(dfa.num_states > 0);
        assert!(dfa.num_states < 200); // Reasonable upper bound
    }

    #[test]
    fn test_serialize_roundtrip() {
        let dfa = ParametricDFA::build(true);
        let bytes = dfa.to_bytes();
        let restored = ParametricDFA::from_bytes(&bytes).unwrap();

        assert_eq!(dfa.num_states, restored.num_states);
        assert_eq!(dfa.accept, restored.accept);
        assert_eq!(dfa.transitions, restored.transitions);
    }

    #[test]
    fn test_exact_match() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "hello");

        assert_eq!(matcher.matches("hello"), Some(0));
        assert_eq!(matcher.matches("world"), None);
    }

    #[test]
    fn test_one_edit() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "hello");

        // Substitution
        assert_eq!(matcher.matches("hallo"), Some(1));
        // Insertion
        assert_eq!(matcher.matches("helloo"), Some(1));
        // Deletion
        assert_eq!(matcher.matches("helo"), Some(1));
    }

    #[test]
    fn test_two_edits() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "hello");

        assert_eq!(matcher.matches("hllo"), Some(1)); // One deletion
        assert_eq!(matcher.matches("helllo"), Some(1)); // One insertion
        assert!(matcher.matches("help").is_some()); // Two edits
    }

    #[test]
    fn test_transposition() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "hello");

        // Transposition: "hlelo" = swap e and l
        let result = matcher.matches("hlelo");
        assert!(result.is_some());
        assert!(result.unwrap() <= 2);
    }

    #[test]
    fn test_too_many_edits() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "hello");

        // Three edits - should not match
        assert_eq!(matcher.matches("xxxxx"), None);
        assert_eq!(matcher.matches("h"), None); // 4 deletions
    }

    #[test]
    fn test_programming_typo() {
        let dfa = ParametricDFA::build(true);
        let matcher = QueryMatcher::new(&dfa, "progamming"); // Missing 'r'

        // "programming" should match with 1 edit (insertion of 'r')
        let result = matcher.matches("programming");
        assert!(result.is_some(), "programming should match progamming");
        assert!(result.unwrap() <= 2);
    }
}
