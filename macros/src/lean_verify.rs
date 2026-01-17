// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! The `#[lean_verify]` attribute macro.
//!
//! Add `requires` and `ensures` clauses to a function, get a Lean theorem
//! statement with `sorry` where the proof should go. The mechanical part
//! is automated. The proof is your problem.
//!
//! This is the seam between "I think this is correct" and "I can prove it."

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, FnArg, Ident, ItemFn, Lit, Meta, Pat,
    ReturnType, Token,
};

use crate::codegen::rust_type_to_lean;

/// Parsed attributes from `#[lean_verify(...)]`.
#[derive(Default, Debug)]
struct LeanVerifyAttrs {
    /// Specification name (used for theorem naming)
    spec: Option<String>,
    /// Precondition (Lean `Prop`)
    requires: Option<String>,
    /// Postcondition (Lean `Prop`)
    ensures: Option<String>,
    /// Additional properties to prove
    properties: Vec<String>,
    /// Whether to generate a `sorry` proof placeholder
    generate_sorry: bool,
}

impl LeanVerifyAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result = LeanVerifyAttrs {
            generate_sorry: true,
            ..Default::default()
        };

        if attr.is_empty() {
            return Ok(result);
        }

        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        let nested = parser.parse(attr)?;

        for meta in nested {
            match meta {
                Meta::NameValue(nv) => {
                    let key = nv.path.get_ident().map(|i| i.to_string());
                    match &nv.value {
                        syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(lit_str),
                            ..
                        }) => match key.as_deref() {
                            Some("spec") => result.spec = Some(lit_str.value()),
                            Some("requires") => result.requires = Some(lit_str.value()),
                            Some("ensures") => result.ensures = Some(lit_str.value()),
                            _ => {}
                        },
                        syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Bool(lit_bool),
                            ..
                        }) => {
                            if key.as_deref() == Some("generate_sorry") {
                                result.generate_sorry = lit_bool.value;
                            }
                        }
                        _ => {}
                    }
                }
                Meta::List(list) if list.path.is_ident("properties") => {
                    // Parse properties = ["prop1", "prop2"]
                    let nested: syn::punctuated::Punctuated<Lit, Token![,]> =
                        list.parse_args_with(Punctuated::parse_terminated)?;
                    for lit in nested {
                        if let Lit::Str(s) = lit {
                            result.properties.push(s.value());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(result)
    }
}

/// Generate a Lean function signature from a Rust function.
fn generate_lean_signature(func: &ItemFn) -> String {
    let name = func.sig.ident.to_string();
    let mut params = Vec::new();

    // Process parameters
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pat_type) => {
                let param_name = match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    _ => "_".to_string(),
                };
                let lean_type = rust_type_to_lean(&pat_type.ty);
                params.push(format!("({} : {})", param_name, lean_type));
            }
            FnArg::Receiver(_) => {
                params.push("(self : Self)".to_string());
            }
        }
    }

    // Process return type
    let return_type = match &func.sig.output {
        ReturnType::Default => "Unit".to_string(),
        ReturnType::Type(_, ty) => rust_type_to_lean(ty),
    };

    format!("def {} {} : {} :=", name, params.join(" "), return_type)
}

/// Generate Lean theorem statements for the function.
fn generate_lean_theorems(func: &ItemFn, attrs: &LeanVerifyAttrs) -> String {
    let mut lean_code = String::new();
    let func_name = func.sig.ident.to_string();
    let spec_name = attrs.spec.clone().unwrap_or_else(|| func_name.clone());

    // Generate precondition theorem
    if let Some(requires) = &attrs.requires {
        lean_code.push_str(&format!(
            "\n/-- Precondition for `{}` -/\n",
            func_name
        ));
        lean_code.push_str(&format!(
            "def {}_requires : Prop :=\n  {}\n",
            spec_name, requires
        ));
    }

    // Generate postcondition theorem
    if let Some(ensures) = &attrs.ensures {
        lean_code.push_str(&format!(
            "\n/-- Postcondition for `{}` -/\n",
            func_name
        ));
        lean_code.push_str(&format!(
            "def {}_ensures : Prop :=\n  {}\n",
            spec_name, ensures
        ));

        // Generate the main correctness theorem
        lean_code.push_str(&format!(
            "\n/-- {} satisfies its specification -/\n",
            func_name
        ));

        // Build parameter list for theorem
        let params: Vec<String> = func
            .sig
            .inputs
            .iter()
            .filter_map(|arg| {
                if let FnArg::Typed(pat_type) = arg {
                    let name = match pat_type.pat.as_ref() {
                        Pat::Ident(ident) => ident.ident.to_string(),
                        _ => return None,
                    };
                    let lean_type = rust_type_to_lean(&pat_type.ty);
                    Some(format!("({} : {})", name, lean_type))
                } else {
                    None
                }
            })
            .collect();

        lean_code.push_str(&format!("theorem {}_correct\n", spec_name));

        // Add parameters
        for param in &params {
            lean_code.push_str(&format!("    {}\n", param));
        }

        // Add precondition as hypothesis if present
        if attrs.requires.is_some() {
            lean_code.push_str(&format!("    (h_pre : {}_requires)\n", spec_name));
        }

        lean_code.push_str(&format!("    : {}_ensures := by\n", spec_name));

        if attrs.generate_sorry {
            lean_code.push_str("  sorry\n");
        } else {
            lean_code.push_str("  -- TODO: provide proof\n  sorry\n");
        }
    }

    // Generate additional property theorems
    for prop in &attrs.properties {
        lean_code.push_str(&format!("\n/-- Property: {} -/\n", prop));
        lean_code.push_str(&format!("theorem {}_{} : Prop := by\n", spec_name, prop));
        lean_code.push_str("  sorry\n");
    }

    lean_code
}

/// Generate the full Lean specification for a function.
fn generate_lean_spec(func: &ItemFn, attrs: &LeanVerifyAttrs) -> String {
    let mut lean_code = String::new();
    let func_name = &func.sig.ident;

    // Add header comment
    lean_code.push_str(&format!(
        "/- Specification for Rust function `{}` -/\n\n",
        func_name
    ));

    // Generate signature (as a comment, since we can't fully translate the body)
    lean_code.push_str("/-\n");
    lean_code.push_str(&generate_lean_signature(func));
    lean_code.push_str("\n  -- Implementation translated from Rust\n");
    lean_code.push_str("-/\n");

    // Generate theorems
    lean_code.push_str(&generate_lean_theorems(func, attrs));

    lean_code
}

/// Main entry point for the `#[lean_verify]` attribute macro.
pub fn process(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the function
    let func = parse_macro_input!(item as ItemFn);

    // Parse the attributes
    let attrs = match LeanVerifyAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    // Generate Lean specification
    let lean_spec = generate_lean_spec(&func, &attrs);
    let func_name = &func.sig.ident;

    // Create constants for the generated spec
    let spec_const_name = Ident::new(
        &format!("{}_LEAN_SPEC", func_name.to_string().to_uppercase()),
        func_name.span(),
    );

    // Return the original function plus the spec constant
    let expanded = quote! {
        #func

        /// Generated Lean 4 specification for this function.
        #[doc(hidden)]
        pub const #spec_const_name: &'static str = #lean_spec;
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attrs_parsing() {
        // Integration tests would go here
    }
}
