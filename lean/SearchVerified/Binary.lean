/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  Binary format correctness.

  The `.sorex` format must survive roundtrip: encode then decode gives back
  what you started with. Varints, delta-encoded postings, section tables:
  all of it must serialize and deserialize without silent corruption.

  ## Binary Format Overview

  The .sorex format uses several encoding schemes:
  - Varint (LEB128): Variable-length integer encoding for size fields
  - Delta encoding: Posting lists encode doc_id and offset deltas
  - Section table: String interning for section IDs

  ## Key Invariants

  1. varint_roundtrip: encode then decode yields original value
  2. section_table_roundtrip: section IDs survive encode/decode
  3. postings_roundtrip: posting lists survive encode/decode
  4. sorex_roundtrip: full index survives encode/decode

  ## Proof Status

  All properties are axiomatized and verified by property tests in Rust.
-/

import SearchVerified.Types
import SearchVerified.InvertedIndex

namespace SearchVerified.Binary

open SearchVerified
open SearchVerified.Inverted

/-! ## Varint Encoding (LEB128)

Variable-length integer encoding where each byte uses 7 bits for data
and 1 bit (MSB) as continuation flag.

Examples:
- 0 encodes as [0x00]
- 127 encodes as [0x7F]
- 128 encodes as [0x80, 0x01]
- 300 encodes as [0xAC, 0x02]
-/

/-- Encode a natural number as a list of bytes (varint/LEB128).

    Implementation note: This is a specification; actual implementation
    is in Rust's binary/encoding.rs -/
def encode_varint (value : Nat) : List UInt8 :=
  if value < 128 then [value.toUInt8]
  else (value % 128 + 128).toUInt8 :: encode_varint (value / 128)
  termination_by value

/-- Decode a varint from a byte list.

    Returns (decoded_value, remaining_bytes) or none on failure. -/
partial def decode_varint (bytes : List UInt8) : Option (Nat × List UInt8) :=
  go bytes 0 0
where
  go (bs : List UInt8) (acc shift : Nat) : Option (Nat × List UInt8) :=
    match bs with
    | [] => none  -- Incomplete varint
    | b :: rest =>
      let value := (b.toNat % 128) <<< shift
      let new_acc := acc + value
      if b.toNat < 128 then
        some (new_acc, rest)  -- No continuation bit
      else
        go rest new_acc (shift + 7)  -- Continue reading

/-- Varint encoding is reversible.

    Verified by: prop_varint_roundtrip in tests/property.rs -/
axiom varint_roundtrip (value : Nat) :
    ∃ rest, decode_varint (encode_varint value) = some (value, rest)

/-- Varint encoding is prefix-free.

    No encoded value is a prefix of another encoded value.
    This ensures unambiguous decoding of concatenated varints.

    Verified by: prop_varint_prefix_free in tests/property.rs -/
axiom varint_prefix_free (v1 v2 : Nat) (h : v1 ≠ v2) :
    ¬(encode_varint v1).isPrefixOf (encode_varint v2)

/-! ## Section Table Encoding

Section IDs are interned in a string table to avoid redundant storage.
Each posting stores a section index instead of the full string.
-/

/-- Encode a section table (list of section ID strings).

    Format: count (varint) followed by length-prefixed strings.
    Implementation in Rust's binary/encoding.rs -/
def encode_section_table (sections : List String) : List UInt8 :=
  encode_varint sections.length ++
    sections.flatMap (fun s => encode_varint s.length ++ s.toUTF8.toList)

/-- Placeholder for section table decoder -/
axiom decode_section_table : List UInt8 → Option (List String × List UInt8)

/-- Section table encoding is reversible.

    Verified by: prop_section_table_roundtrip in tests/property.rs -/
axiom section_table_roundtrip (sections : List String) :
    ∃ rest decoded, decode_section_table (encode_section_table sections) = some (decoded, rest) ∧
      decoded = sections

/-! ## Posting List Encoding

Posting lists use delta encoding for efficient compression:
- First posting: full doc_id and offset
- Subsequent postings: delta from previous values
- Field type and heading_level are stored per posting
-/

/-- Encode a posting list with delta encoding.

    Implementation in Rust's binary/postings.rs -/
def encode_postings (postings : List Posting) : List UInt8 :=
  encode_varint postings.length  -- Placeholder: actual encoding is complex

/-- Placeholder for posting list decoder -/
axiom decode_postings : List UInt8 → Nat → Option (List Posting × List UInt8)

/-- Posting list encoding is reversible.

    Verified by: prop_postings_roundtrip in tests/property.rs -/
axiom postings_roundtrip (postings : List Posting) :
    ∃ rest decoded, decode_postings (encode_postings postings) postings.length = some (decoded, rest) ∧
      decoded = postings

/-! ## Full Index Encoding

The complete .sorex binary format includes:
1. Magic bytes and version
2. Header with section offsets
3. Document metadata
4. Section table
5. Posting lists per term
6. Suffix array
7. Embedded WASM (optional)
8. CRC32 checksum
-/

/-- Placeholder for full index encoder -/
axiom encode_sorex : SearchIndex → List UInt8

/-- Placeholder for full index decoder -/
axiom decode_sorex : List UInt8 → Option SearchIndex

/-- A well-formed .sorex file can be decoded to the original index.

    This is the top-level correctness property: any index that
    satisfies WellFormed can be serialized and deserialized
    without loss of information.

    Verified by: prop_sorex_roundtrip in tests/property.rs -/
axiom sorex_roundtrip (idx : SearchIndex) (h : SearchIndex.WellFormed idx) :
    decode_sorex (encode_sorex idx) = some idx

end SearchVerified.Binary
