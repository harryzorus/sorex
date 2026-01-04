import Lake
open Lake DSL

package «SearchVerified» where
  leanOptions := #[
    ⟨`autoImplicit, false⟩,
    ⟨`relaxedAutoImplicit, false⟩
  ]

lean_lib «SearchVerified» where
  roots := #[`SearchVerified]

@[default_target]
lean_exe «check_proofs» where
  root := `Main
  supportInterpreter := true

-- Mathlib for edit distance definitions
require mathlib from git
  "https://github.com/leanprover-community/mathlib4" @ "v4.14.0"
