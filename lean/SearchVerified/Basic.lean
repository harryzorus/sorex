/-
  Basic.lean - Common utilities and lemmas for search verification.

  This module provides shared infrastructure used across all proof modules.
-/

import SearchVerified.Types

namespace SearchVerified

/-! ## Nat Utilities -/

/-- If a < b - 1, then a < b -/
theorem Nat.lt_of_lt_pred {a b : Nat} (h : a < b - 1) : a < b := by
  omega

/-- Subtraction preserves ordering -/
theorem Nat.sub_le_sub_of_le {a b c : Nat} (h : a ≤ b) : a - c ≤ b - c := by
  omega

/-! ## Array Utilities -/

/-- Array element at valid index is in the list representation -/
theorem Array.getElem_mem_toList {α : Type _} (a : Array α) (i : Nat) (h : i < a.size) :
    a[i] ∈ a.toList := by
  simp only [Array.toList, Array.getElem_eq_toList_getElem]
  exact List.getElem_mem h

/-- Array size equals list length -/
theorem Array.size_eq_length {α : Type _} (a : Array α) : a.size = a.toList.length := by
  simp [Array.toList]

/-! ## List Utilities -/

/-- Length of zip is minimum of both lengths -/
theorem List.length_zip_eq_min {α β : Type _} (as : List α) (bs : List β) :
    (as.zip bs).length = min as.length bs.length := by
  simp [List.length_zip]

/-- takeWhile length bounded by original -/
theorem List.length_takeWhile_le {α : Type _} (p : α → Bool) (as : List α) :
    (as.takeWhile p).length ≤ as.length := by
  induction as with
  | nil => simp
  | cons a as ih =>
    simp only [List.takeWhile]
    split
    · simp; omega
    · simp

/-! ## String Utilities -/

/-- String length is list length -/
theorem String.length_eq_data_length (s : String) : s.length = s.data.length := by
  simp [String.length]

end SearchVerified
