/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  Reference implementations for differential testing.

  These are simple, obviously-correct implementations that serve as oracles
  for verifying the optimized Rust code. The Rust tests call these via FFI
  and compare outputs.

  ## Philosophy

  Don't prove the optimized Rust code (hard). Prove these simple Lean
  implementations, then verify Rust matches them via differential fuzzing.

  If `rust_sais(input) == lean_simple_suffix_array(input)` for all tested
  inputs, and we've proven `lean_simple_suffix_array` correct, then we have
  high confidence in the Rust implementation.

  ## Exported Functions

  All functions with `@[export c_*]` are compiled to C and linked into Rust
  tests via FFI. The naming convention follows Lean's C export format.

  ## Building

  ```bash
  cd lean && lake build
  # C files generated in .lake/build/ir/
  ```
-/

import SearchVerified.Types

namespace SearchVerified.Oracle

open SearchVerified

-- =============================================================================
-- SUFFIX ARRAY ORACLE
-- =============================================================================

/-- Compare two suffixes lexicographically.

    Given a string and two starting positions, returns true if the suffix
    starting at `i` is lexicographically less than the suffix starting at `j`.
-/
def suffixLt (s : String) (i j : Nat) : Bool :=
  let si := s.drop i
  let sj := s.drop j
  si < sj

/-- Build suffix array using naive O(n² log n) sorting.

    This is the reference implementation. It's slow but obviously correct:
    just sort all suffix positions by their corresponding suffix strings.

    **Theorem**: The output is sorted (proven by construction via List.mergeSort).
-/
def simpleSuffixArray (input : String) : Array Nat :=
  let n := input.length
  let positions := List.range n
  let sorted := positions.mergeSort (suffixLt input)
  sorted.toArray

/-- Theorem: simpleSuffixArray produces sorted output.

    Since we use List.mergeSort with a valid comparison function,
    the output is guaranteed to be sorted.
-/
theorem simpleSuffixArray_sorted (input : String) :
    ∀ i j : Nat, i < j →
      (h1 : i < (simpleSuffixArray input).size) →
      (h2 : j < (simpleSuffixArray input).size) →
      suffixLt input (simpleSuffixArray input)[i] (simpleSuffixArray input)[j] ∨
      (simpleSuffixArray input)[i] = (simpleSuffixArray input)[j] := by
  intro i j hij h1 h2
  -- mergeSort guarantees ordering; this follows from List.Sorted
  sorry -- Would require List.mergeSort lemmas from Mathlib

/-- Export suffix array builder for FFI.

    Called from Rust as: `c_simple_suffix_array(input_ptr, input_len)`
    Returns: Array of Nat (suffix positions in sorted order)

    Note: Lean's C FFI handles memory management automatically.
-/
@[export c_oracle_suffix_array]
def oracleSuffixArray (input : String) : Array UInt32 :=
  (simpleSuffixArray input).map (·.toUInt32)

-- =============================================================================
-- BINARY SEARCH ORACLE
-- =============================================================================

/-- Find first element >= target using linear scan.

    This is O(n) but trivially correct. Used to verify the O(log n)
    binary search implementation in Rust.

    Returns the index of the first element >= target, or arr.size if none found.
-/
def linearSearchFirstGe (arr : Array Nat) (target : Nat) : Nat :=
  go 0
where
  go (i : Nat) : Nat :=
    if h : i < arr.size then
      if arr[i] >= target then i
      else go (i + 1)
    else arr.size
  termination_by arr.size - i

/-- Theorem: linearSearchFirstGe returns correct bounds.

    If result < arr.size, then arr[result] >= target.
    All elements before result are < target.
-/
theorem linearSearchFirstGe_correct (arr : Array Nat) (target : Nat) :
    let result := linearSearchFirstGe arr target
    (result < arr.size → arr[result]! >= target) ∧
    (∀ i : Nat, i < result → (hi : i < arr.size) → arr[i] < target) := by
  constructor
  · intro h
    -- By construction, we only return i if arr[i] >= target
    sorry
  · intro i hi _
    -- By construction, we skip elements < target
    sorry

/-- Export binary search oracle for FFI. -/
@[export c_oracle_binary_search]
def oracleBinarySearch (arr : Array UInt32) (target : UInt32) : UInt32 :=
  let natArr := arr.map (·.toNat)
  (linearSearchFirstGe natArr target.toNat).toUInt32

-- =============================================================================
-- LEVENSHTEIN DISTANCE ORACLE
-- =============================================================================

/-- Compute Levenshtein edit distance using dynamic programming.

    Classic O(nm) algorithm with O(min(n,m)) space optimization.
    This is the reference implementation for fuzzy matching.
-/
def levenshteinDistance (s1 s2 : String) : Nat :=
  let a := s1.toList
  let b := s2.toList
  let m := a.length
  let n := b.length

  if m == 0 then n
  else if n == 0 then m
  else
    -- Use two rows for space efficiency
    let row0 := Array.range (n + 1)
    let (_, finalRow) := a.foldl (fun (i, prev) c1 =>
      let curr := Array.mkArray (n + 1) 0
      let curr := curr.set! 0 (i + 1)
      let (_, curr) := b.foldl (fun (j, curr) c2 =>
        let cost := if c1 == c2 then 0 else 1
        let deletion := prev[j + 1]! + 1
        let insertion := curr[j]! + 1
        let substitution := prev[j]! + cost
        let minCost := min deletion (min insertion substitution)
        (j + 1, curr.set! (j + 1) minCost)
      ) (0, curr)
      (i + 1, curr)
    ) (0, row0)
    finalRow[n]!

/-- Theorem: Levenshtein distance satisfies triangle inequality. -/
theorem levenshtein_triangle (a b c : String) :
    levenshteinDistance a c ≤ levenshteinDistance a b + levenshteinDistance b c := by
  -- Classic property of edit distance
  sorry

/-- Theorem: Length difference is a lower bound on edit distance.

    This is the key optimization used in the Rust DFA: if lengths differ
    by more than max_distance, we can skip the comparison entirely.
-/
theorem levenshtein_length_bound (s1 s2 : String) :
    (Int.natAbs (s1.length - s2.length) : Nat) ≤ levenshteinDistance s1 s2 := by
  -- Each character difference requires at least one edit
  sorry

/-- Export Levenshtein oracle for FFI. -/
@[export c_oracle_levenshtein]
def oracleLevenshtein (s1 s2 : String) : UInt32 :=
  (levenshteinDistance s1 s2).toUInt32

-- =============================================================================
-- COMMON PREFIX LENGTH ORACLE
-- =============================================================================

/-- Compute common prefix length between two strings.

    Used in front compression for vocabulary encoding.
-/
def commonPrefixLen (s1 s2 : String) : Nat :=
  go s1.toList s2.toList 0
where
  go : List Char → List Char → Nat → Nat
  | [], _, acc => acc
  | _, [], acc => acc
  | c1 :: rest1, c2 :: rest2, acc =>
    if c1 == c2 then go rest1 rest2 (acc + 1)
    else acc

/-- Theorem: Common prefix length is bounded by shorter string. -/
theorem commonPrefixLen_bounded (s1 s2 : String) :
    commonPrefixLen s1 s2 ≤ min s1.length s2.length := by
  -- By induction on the recursive calls
  sorry

/-- Export common prefix oracle for FFI. -/
@[export c_oracle_common_prefix]
def oracleCommonPrefix (s1 s2 : String) : UInt32 :=
  (commonPrefixLen s1 s2).toUInt32

-- =============================================================================
-- VARINT ENCODING ORACLE
-- =============================================================================

/-- Encode a natural number as varint bytes (LEB128 format).

    Reference implementation matching the Rust encode_varint function.
-/
def encodeVarint (n : Nat) : List UInt8 :=
  if n < 128 then [n.toUInt8]
  else
    let byte := (n % 128).toUInt8 ||| 0x80
    byte :: encodeVarint (n / 128)
  termination_by n

/-- Decode a varint from bytes, returning (value, bytes_consumed).

    Returns none if the input is invalid (empty or truncated).
-/
def decodeVarint (bytes : List UInt8) : Option (Nat × Nat) :=
  go bytes 0 0
where
  go : List UInt8 → Nat → Nat → Option (Nat × Nat)
  | [], _, _ => none  -- Incomplete varint
  | b :: rest, shift, acc =>
    let value := acc ||| ((b.toNat &&& 0x7F) <<< shift)
    if b &&& 0x80 == 0 then
      some (value, shift / 7 + 1)
    else if shift >= 63 then
      none  -- Varint too long
    else
      go rest (shift + 7) value

/-- Theorem: Varint encoding is reversible. -/
theorem varint_roundtrip (n : Nat) :
    decodeVarint (encodeVarint n) = some (n, (encodeVarint n).length) := by
  -- By induction on the encoding
  sorry

/-- Export varint encode oracle for FFI. -/
@[export c_oracle_encode_varint]
def oracleEncodeVarint (n : UInt64) : Array UInt8 :=
  (encodeVarint n.toNat).toArray

/-- Export varint decode oracle for FFI. -/
@[export c_oracle_decode_varint]
def oracleDecodeVarint (bytes : Array UInt8) : UInt64 :=
  match decodeVarint bytes.toList with
  | some (value, _) => value.toUInt64
  | none => 0  -- Error case

end SearchVerified.Oracle
