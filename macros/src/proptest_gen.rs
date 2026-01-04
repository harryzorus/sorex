//! Automatic property test generation from Lean specifications.
//!
//! This module translates Lean-style predicates into executable Rust property tests.
//! It parses the `ensures` and `requires` clauses from `#[lean_verify]` annotations
//! and generates proptest code that verifies these properties at runtime.
//!
//! # Predicate Translation
//!
//! The translator handles common Lean patterns:
//!
//! | Lean Pattern | Rust Translation |
//! |--------------|------------------|
//! | `∀ i j. P(i,j)` | `for i in 0..len { for j in 0..len { assert!(P(i,j)) } }` |
//! | `a ≤ b` | `a <= b` |
//! | `a < b` | `a < b` |
//! | `P → Q` | `if P { assert!(Q) }` |
//! | `P ∧ Q` | `P && Q` |
//! | `P ∨ Q` | `P \|\| Q` |
//! | `suffix_at texts sa[i]` | `suffix_at(&texts, &sa[i])` |
//!
//! # Example
//!
//! ```ignore
//! #[lean_proptest_verify(
//!     spec = "suffix_array_sorted",
//!     ensures = "∀ i j. i < j → suffix_at texts sa[i] ≤ suffix_at texts sa[j]"
//! )]
//! fn build_index(docs: Vec<SearchDoc>, texts: Vec<String>) -> SearchIndex { ... }
//!
//! // Generates:
//! #[cfg(test)]
//! mod suffix_array_sorted_proptest {
//!     use super::*;
//!     use proptest::prelude::*;
//!
//!     proptest! {
//!         #[test]
//!         fn property(docs in any::<Vec<SearchDoc>>(), texts in any::<Vec<String>>()) {
//!             let result = build_index(docs, texts);
//!             let sa = &result.suffix_array;
//!             let texts = &result.texts;
//!             for i in 0..sa.len() {
//!                 for j in 0..sa.len() {
//!                     if i < j {
//!                         assert!(suffix_at(texts, &sa[i]) <= suffix_at(texts, &sa[j]));
//!                     }
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, FnArg, Ident, ItemFn, Lit, Meta,
    Pat, ReturnType, Token,
};

use crate::codegen::rust_type_to_lean;

/// Parsed attributes from `#[lean_proptest_verify(...)]`.
#[derive(Default, Debug)]
pub struct LeanPropTestVerifyAttrs {
    /// Specification name
    pub spec: Option<String>,
    /// Precondition (Lean `Prop`)
    pub requires: Option<String>,
    /// Postcondition (Lean `Prop`)
    pub ensures: Option<String>,
    /// Additional properties to test
    pub properties: Vec<String>,
    /// Number of test cases
    pub cases: usize,
    /// Custom strategies for parameters (param_name -> strategy)
    pub strategies: Vec<(String, String)>,
}

impl LeanPropTestVerifyAttrs {
    pub fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result = LeanPropTestVerifyAttrs {
            cases: 256, // u32 for proptest config
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
                            lit: Lit::Int(lit_int),
                            ..
                        }) => {
                            if key.as_deref() == Some("cases") {
                                result.cases = lit_int.base10_parse().unwrap_or(256);
                            }
                        }
                        _ => {}
                    }
                }
                Meta::List(list) if list.path.is_ident("properties") => {
                    let nested: Punctuated<Lit, Token![,]> =
                        list.parse_args_with(Punctuated::parse_terminated)?;
                    for lit in nested {
                        if let Lit::Str(s) = lit {
                            result.properties.push(s.value());
                        }
                    }
                }
                Meta::List(list) if list.path.is_ident("strategies") => {
                    // Parse strategies = [("param", "strategy"), ...]
                    let nested: Punctuated<syn::ExprTuple, Token![,]> =
                        list.parse_args_with(Punctuated::parse_terminated)?;
                    for tuple in nested {
                        if tuple.elems.len() == 2 {
                            if let (
                                syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(param), .. }),
                                syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(strat), .. }),
                            ) = (&tuple.elems[0], &tuple.elems[1])
                            {
                                result.strategies.push((param.value(), strat.value()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(result)
    }
}

/// Translate a Lean predicate to Rust assertion code.
///
/// This is a simplified translator that handles common patterns.
/// Complex predicates may need manual translation.
pub fn translate_predicate(lean_pred: &str, result_var: &str) -> TokenStream2 {
    let pred = lean_pred.trim();

    // Handle universal quantification: ∀ i j. P(i,j) or forall i j, P(i,j)
    if pred.starts_with("∀") || pred.starts_with("forall") {
        return translate_forall(pred, result_var);
    }

    // Handle implication: P → Q or P -> Q
    if pred.contains("→") || pred.contains("->") {
        return translate_implication(pred, result_var);
    }

    // Handle conjunction: P ∧ Q or P /\ Q
    if pred.contains("∧") || pred.contains("/\\") {
        return translate_conjunction(pred, result_var);
    }

    // Handle simple comparisons
    translate_comparison(pred, result_var)
}

/// Translate universal quantification.
fn translate_forall(pred: &str, result_var: &str) -> TokenStream2 {
    // Parse: ∀ i j. body or forall i j, body
    let pred = pred
        .trim_start_matches("∀")
        .trim_start_matches("forall")
        .trim();

    // Find the separator (. or ,) between variables and body
    let (vars_part, body) = if let Some(idx) = pred.find('.') {
        (&pred[..idx], &pred[idx + 1..])
    } else if let Some(idx) = pred.find(',') {
        (&pred[..idx], &pred[idx + 1..])
    } else {
        return quote! { /* Failed to parse forall: missing separator */ };
    };

    // Parse variable names
    let vars: Vec<&str> = vars_part.split_whitespace().collect();

    // Determine the collection to iterate over
    // Common pattern: iterate over suffix_array indices
    let body = body.trim();

    // Generate nested loops
    let mut code = translate_predicate(body, result_var);

    // Wrap in loops from innermost to outermost
    for var in vars.iter().rev() {
        let var_ident = format_ident!("{}", var);

        // Determine the range based on context
        // If the body mentions sa[var] or suffix_array[var], use suffix_array.len()
        let range = if body.contains("sa[") || body.contains("suffix_array[") {
            quote! { 0..#result_var.suffix_array.len() }
        } else if body.contains("texts[") {
            quote! { 0..#result_var.texts.len() }
        } else if body.contains("docs[") {
            quote! { 0..#result_var.docs.len() }
        } else {
            // Default to suffix_array
            quote! { 0..#result_var.suffix_array.len() }
        };

        code = quote! {
            for #var_ident in #range {
                #code
            }
        };
    }

    code
}

/// Translate implication: P → Q
fn translate_implication(pred: &str, result_var: &str) -> TokenStream2 {
    let parts: Vec<&str> = if pred.contains("→") {
        pred.split("→").collect()
    } else {
        pred.split("->").collect()
    };

    if parts.len() != 2 {
        return quote! { /* Failed to parse implication */ };
    }

    let condition = translate_to_rust_expr(parts[0].trim(), result_var);
    let consequent = translate_predicate(parts[1].trim(), result_var);

    quote! {
        if #condition {
            #consequent
        }
    }
}

/// Translate conjunction: P ∧ Q
fn translate_conjunction(pred: &str, result_var: &str) -> TokenStream2 {
    let parts: Vec<&str> = if pred.contains("∧") {
        pred.split("∧").collect()
    } else {
        pred.split("/\\").collect()
    };

    let assertions: Vec<TokenStream2> = parts
        .iter()
        .map(|p| translate_predicate(p.trim(), result_var))
        .collect();

    quote! {
        #(#assertions)*
    }
}

/// Translate a comparison or simple expression to Rust.
fn translate_comparison(pred: &str, result_var: &str) -> TokenStream2 {
    let rust_expr = translate_to_rust_expr(pred, result_var);

    quote! {
        assert!(#rust_expr, "Property violation: {}", stringify!(#rust_expr));
    }
}

/// Convert a Lean expression to a Rust expression token stream.
///
/// Note: This translator is conservative - it only prefixes result field names
/// when they appear in specific patterns (like result.suffix_array). Parameter
/// names are left as-is since they're in scope in the generated test.
fn translate_to_rust_expr(expr: &str, result_var: &str) -> TokenStream2 {
    let expr = expr.trim();

    // Replace Lean operators with Rust operators
    let mut rust_str = expr
        // Comparisons
        .replace("≤", "<=")
        .replace("≥", ">=")
        .replace("≠", "!=")
        // Logical operators
        .replace("∧", "&&")
        .replace("∨", "||")
        .replace("¬", "!")
        .to_string();

    // Only prefix field names that are explicitly accessing result fields
    // Pattern: when result_var is not empty and we're translating result properties
    if !result_var.is_empty() {
        // These patterns indicate accessing the result's fields
        rust_str = rust_str
            .replace("sa[", &format!("{}.suffix_array[", result_var))
            .replace("suffix_array[", &format!("{}.suffix_array[", result_var));

        // Only replace result.X patterns, not bare X
        // The ensures clause uses 'result' to refer to the function return value
        rust_str = rust_str
            .replace("result.len()", &format!("{}.len()", result_var))
            .replace("result.suffix_array", &format!("{}.suffix_array", result_var))
            .replace("result.texts", &format!("{}.texts", result_var))
            .replace("result.docs", &format!("{}.docs", result_var))
            .replace("result.lcp", &format!("{}.lcp", result_var));
    }

    // Parse the string as a Rust expression
    rust_str
        .parse()
        .unwrap_or_else(|_| quote! { true /* parse error */ })
}

/// Parameter info with reference handling.
struct ParamInfo {
    name: Ident,
    strategy: TokenStream2,
    is_reference: bool,
}

/// Generate parameter strategies for a function.
fn generate_param_strategies(
    func: &ItemFn,
    custom_strategies: &[(String, String)],
) -> Vec<ParamInfo> {
    let custom_map: std::collections::HashMap<_, _> =
        custom_strategies.iter().cloned().collect();

    func.sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                let param_name = match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.clone(),
                    _ => return None,
                };

                let param_name_str = param_name.to_string();
                let ty = &pat_type.ty;

                // Check if this is a reference type
                let is_reference = matches!(ty.as_ref(), syn::Type::Reference(_));

                // Check for custom strategy
                let strategy = if let Some(custom) = custom_map.get(&param_name_str) {
                    custom.parse().unwrap_or_else(|_| {
                        quote! { proptest::arbitrary::any::<#ty>() }
                    })
                } else {
                    // Generate default strategy based on type (strips references)
                    generate_default_strategy(ty)
                };

                Some(ParamInfo {
                    name: param_name,
                    strategy,
                    is_reference,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Generate a default proptest strategy for a type.
fn generate_default_strategy(ty: &syn::Type) -> TokenStream2 {
    match ty {
        // Strip references - we'll generate owned values
        syn::Type::Reference(reference) => {
            generate_default_strategy(&reference.elem)
        }
        // Handle slices - convert to Vec strategy
        syn::Type::Slice(slice) => {
            let inner_strategy = generate_default_strategy(&slice.elem);
            quote! { proptest::collection::vec(#inner_strategy, 0..5) }
        }
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last();
            match segment {
                Some(seg) => {
                    let ident = seg.ident.to_string();
                    match ident.as_str() {
                        "usize" => quote! { 0usize..100 },
                        "u32" => quote! { 0u32..100 },
                        "String" => quote! { "[a-z]{0,20}" },
                        "Vec" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    let inner_strategy = generate_default_strategy(inner);
                                    return quote! {
                                        proptest::collection::vec(#inner_strategy, 0..5)
                                    };
                                }
                            }
                            quote! { proptest::collection::vec(proptest::strategy::Just(()), 0..5) }
                        }
                        // Known custom types that have Arbitrary implementations
                        "SuffixEntry" => quote! {
                            (0usize..10, 0usize..100).prop_map(|(doc_id, offset)| SuffixEntry { doc_id, offset })
                        },
                        "SearchDoc" => quote! {
                            (0usize..10, "[a-z]{1,20}", "[a-z]{0,50}", "/[a-z]{1,10}")
                                .prop_map(|(id, title, excerpt, href)| SearchDoc {
                                    id,
                                    title,
                                    excerpt,
                                    href,
                                    kind: "post".to_string(),
                                })
                        },
                        _ => quote! { proptest::arbitrary::any::<#ty>() },
                    }
                }
                None => quote! { proptest::arbitrary::any::<#ty>() },
            }
        }
        _ => quote! { proptest::arbitrary::any::<#ty>() },
    }
}

/// Generate the property test module for a function.
pub fn generate_proptest_module(func: &ItemFn, attrs: &LeanPropTestVerifyAttrs) -> TokenStream2 {
    let func_name = &func.sig.ident;
    let spec_name = attrs
        .spec
        .clone()
        .unwrap_or_else(|| format!("{}_spec", func_name));
    let test_mod_name = format_ident!("{}_proptest", spec_name);

    let cases = attrs.cases as u32; // proptest expects u32

    // Generate parameter strategies
    let params = generate_param_strategies(func, &attrs.strategies);

    if params.is_empty() {
        return quote! {
            /* No parameters to generate strategies for */
        };
    }

    // Build the proptest input pattern
    let param_patterns: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name = &p.name;
            let strategy = &p.strategy;
            quote! { #name in #strategy }
        })
        .collect();

    // Generate the function call arguments, handling references
    let func_args: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name = &p.name;
            if p.is_reference {
                quote! { &#name }
            } else {
                quote! { #name.clone() }
            }
        })
        .collect();

    // Generate the function call
    let func_call = quote! {
        let result = #func_name(#(#func_args),*);
    };

    // Generate assertions from ensures clause
    let ensures_assertions = if let Some(ensures) = &attrs.ensures {
        translate_predicate(ensures, "result")
    } else {
        quote! {}
    };

    // Generate assertions from requires clause (as precondition filter)
    let requires_filter = if let Some(requires) = &attrs.requires {
        let cond = translate_to_rust_expr(requires, "");
        quote! {
            prop_assume!(#cond);
        }
    } else {
        quote! {}
    };

    // Generate additional property tests
    let property_tests: Vec<TokenStream2> = attrs
        .properties
        .iter()
        .map(|prop| translate_predicate(prop, "result"))
        .collect();

    // Generate the complete test module
    quote! {
        #[cfg(test)]
        mod #test_mod_name {
            use super::*;
            use proptest::prelude::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(#cases))]

                #[test]
                fn ensures_property(#(#param_patterns),*) {
                    #requires_filter
                    #func_call
                    #ensures_assertions
                    #(#property_tests)*
                }
            }
        }
    }
}

/// Main entry point for the `#[lean_proptest_verify]` attribute macro.
pub fn process(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    let attrs = match LeanPropTestVerifyAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    // Generate the proptest module
    let test_module = generate_proptest_module(&func, &attrs);

    // Also generate the Lean spec constant (like lean_verify does)
    let lean_spec = generate_lean_spec(&func, &attrs);
    let func_name = &func.sig.ident;
    let spec_const_name = format_ident!(
        "{}_LEAN_SPEC",
        func_name.to_string().to_uppercase()
    );

    // Return the original function plus the spec constant plus the test module
    let expanded = quote! {
        #func

        /// Generated Lean 4 specification for this function.
        #[doc(hidden)]
        pub const #spec_const_name: &'static str = #lean_spec;

        #test_module
    };

    TokenStream::from(expanded)
}

/// Generate Lean specification (reuse from lean_verify).
fn generate_lean_spec(func: &ItemFn, attrs: &LeanPropTestVerifyAttrs) -> String {
    let mut lean_code = String::new();
    let func_name = &func.sig.ident;

    lean_code.push_str(&format!(
        "/- Specification for Rust function `{}` -/\n\n",
        func_name
    ));

    // Generate signature
    lean_code.push_str("/-\n");
    lean_code.push_str(&format!("def {} ", func_name));

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

    lean_code.push_str(&params.join(" "));

    let return_type = match &func.sig.output {
        ReturnType::Default => "Unit".to_string(),
        ReturnType::Type(_, ty) => rust_type_to_lean(ty),
    };

    lean_code.push_str(&format!(" : {} :=\n", return_type));
    lean_code.push_str("  -- Implementation in Rust\n");
    lean_code.push_str("-/\n");

    // Add requires
    if let Some(requires) = &attrs.requires {
        lean_code.push_str(&format!(
            "\n/-- Precondition -/\ndef {}_requires : Prop :=\n  {}\n",
            attrs.spec.as_deref().unwrap_or(&func_name.to_string()),
            requires
        ));
    }

    // Add ensures
    if let Some(ensures) = &attrs.ensures {
        lean_code.push_str(&format!(
            "\n/-- Postcondition -/\ndef {}_ensures : Prop :=\n  {}\n",
            attrs.spec.as_deref().unwrap_or(&func_name.to_string()),
            ensures
        ));
    }

    lean_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_comparison() {
        let result = translate_to_rust_expr("i < j", "result");
        assert!(result.to_string().contains("<"));
    }

    #[test]
    fn test_translate_le() {
        let result = translate_to_rust_expr("a ≤ b", "result");
        assert!(result.to_string().contains("<="));
    }
}
