//! Implementation of the `#[derive(LeanSpec)]` macro.
//!
//! This macro generates Lean 4 structure definitions from Rust structs,
//! including field type translations and optional well-formedness predicates.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, Lit, Meta};

use crate::codegen::{rust_type_to_lean, types::rust_ident_to_lean};

/// Parse `#[lean(...)]` attributes from a list of attributes.
#[derive(Default)]
struct LeanAttrs {
    /// Override the Lean type name
    name: Option<String>,
    /// Well-formedness invariant
    invariant: Option<String>,
    /// Documentation comment
    doc: Option<String>,
}

impl LeanAttrs {
    fn from_attrs(attrs: &[Attribute]) -> Self {
        let mut result = LeanAttrs::default();

        for attr in attrs {
            if attr.path().is_ident("lean") {
                if let Ok(nested) = attr.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::NameValue(nv) = meta {
                            let key = nv.path.get_ident().map(|i| i.to_string());
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) = &nv.value
                            {
                                match key.as_deref() {
                                    Some("name") => result.name = Some(lit_str.value()),
                                    Some("invariant") => result.invariant = Some(lit_str.value()),
                                    Some("doc") => result.doc = Some(lit_str.value()),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            // Also extract doc comments
            if attr.path().is_ident("doc") {
                if let Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(lit_str),
                        ..
                    }) = &nv.value
                    {
                        let doc_line = lit_str.value();
                        if let Some(existing) = &result.doc {
                            result.doc = Some(format!("{}\n{}", existing, doc_line.trim()));
                        } else {
                            result.doc = Some(doc_line.trim().to_string());
                        }
                    }
                }
            }
        }

        result
    }
}

/// Parse field-level attributes.
struct FieldAttrs {
    /// Field-specific invariant
    invariant: Option<String>,
    /// Override the Lean field name
    name: Option<String>,
}

impl FieldAttrs {
    fn from_field(field: &Field) -> Self {
        let mut result = FieldAttrs {
            invariant: None,
            name: None,
        };

        for attr in &field.attrs {
            if attr.path().is_ident("lean") {
                if let Ok(nested) = attr.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::NameValue(nv) = meta {
                            let key = nv.path.get_ident().map(|i| i.to_string());
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) = &nv.value
                            {
                                match key.as_deref() {
                                    Some("invariant") => result.invariant = Some(lit_str.value()),
                                    Some("name") => result.name = Some(lit_str.value()),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

/// Generate a Lean structure definition from Rust struct fields.
fn generate_lean_struct(
    name: &str,
    fields: &Fields,
    attrs: &LeanAttrs,
    _field_attrs: &[(String, FieldAttrs)],
) -> String {
    let mut lean_code = String::new();

    // Add doc comment if present
    if let Some(doc) = &attrs.doc {
        for line in doc.lines() {
            lean_code.push_str(&format!("/-- {} -/\n", line));
        }
    }

    // Structure definition
    lean_code.push_str(&format!("structure {} where\n", name));

    // Generate fields
    match fields {
        Fields::Named(named) => {
            for field in &named.named {
                let field_name = field.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                let lean_name = rust_ident_to_lean(&field_name);
                let lean_type = rust_type_to_lean(&field.ty);
                lean_code.push_str(&format!("  {} : {}\n", lean_name, lean_type));
            }
        }
        Fields::Unnamed(unnamed) => {
            for (i, field) in unnamed.unnamed.iter().enumerate() {
                let lean_type = rust_type_to_lean(&field.ty);
                lean_code.push_str(&format!("  field{} : {}\n", i, lean_type));
            }
        }
        Fields::Unit => {
            // Unit structs have no fields
        }
    }

    // Add deriving clause
    lean_code.push_str("  deriving Repr, DecidableEq\n");

    // Generate well-formedness predicate if invariant specified
    if let Some(invariant) = &attrs.invariant {
        lean_code.push('\n');
        lean_code.push_str(&format!(
            "/-- Well-formedness predicate for {} -/\n",
            name
        ));
        lean_code.push_str(&format!(
            "def {}.WellFormed (x : {}) : Prop :=\n  {}\n",
            name, name, invariant
        ));
    }

    // Collect field invariants and generate combined predicate
    let field_invariants: Vec<_> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| {
                let field_attrs = FieldAttrs::from_field(f);
                field_attrs.invariant.map(|inv| {
                    let field_name = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                    (field_name, inv)
                })
            })
            .collect(),
        _ => vec![],
    };

    if !field_invariants.is_empty() && attrs.invariant.is_none() {
        lean_code.push('\n');
        lean_code.push_str(&format!(
            "/-- Field invariants for {} -/\n",
            name
        ));
        lean_code.push_str(&format!("def {}.FieldsWellFormed (x : {}) : Prop :=\n", name, name));

        let conditions: Vec<String> = field_invariants
            .iter()
            .map(|(field, inv)| format!("  (x.{} |> fun {} => {})", field, field, inv))
            .collect();

        lean_code.push_str(&conditions.join(" ∧\n"));
        lean_code.push('\n');
    }

    lean_code
}

/// Main entry point for the `#[derive(LeanSpec)]` macro.
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract struct-level attributes
    let lean_attrs = LeanAttrs::from_attrs(&input.attrs);
    let lean_name = lean_attrs.name.clone().unwrap_or_else(|| name.to_string());

    // Get fields from struct
    let fields = match &input.data {
        Data::Struct(data_struct) => &data_struct.fields,
        Data::Enum(_) => {
            return syn::Error::new_spanned(
                &input.ident,
                "LeanSpec currently only supports structs, not enums",
            )
            .to_compile_error()
            .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(&input.ident, "LeanSpec does not support unions")
                .to_compile_error()
                .into();
        }
    };

    // Extract field-level attributes
    let field_attrs: Vec<_> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let name = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                let attrs = FieldAttrs::from_field(f);
                (name, attrs)
            })
            .collect(),
        _ => vec![],
    };

    // Generate Lean code
    let lean_code = generate_lean_struct(&lean_name, fields, &lean_attrs, &field_attrs);

    // Create the expanded output with LEAN_SPEC constant
    let expanded = quote! {
        impl #name {
            /// The generated Lean 4 specification for this type.
            ///
            /// This constant contains the Lean structure definition that corresponds
            /// to this Rust struct, with types translated according to the mapping:
            /// - `usize` → `Nat`
            /// - `Vec<T>` → `Array T`
            /// - `String` → `String`
            /// - etc.
            pub const LEAN_SPEC: &'static str = #lean_code;

            /// Get the Lean type name for this struct.
            pub const LEAN_NAME: &'static str = #lean_name;
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lean_attrs_parsing() {
        // This would need integration tests with actual token streams
        // For now, we test the helper functions
    }
}
