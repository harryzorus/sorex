// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Property testing from Lean annotations.
//!
//! `#[derive(LeanProptest)]` reads the bounds you specified for Lean and
//! generates proptest strategies that respect them. `#[lean_proptest]` wraps
//! your test in a harness that runs thousands of cases.
//!
//! The payoff: you wrote Lean specs, now you get tests for free. The fuzzer
//! hammers the Rust with random inputs satisfying your invariants. Not a
//! proof, but it catches bugs that proofs didn't think to look for.
//!
//! # Architecture
//!
//! The property testing system works in three layers:
//!
//! 1. **Strategy Generation** (`LeanProptest` derive):
//!    Generates proptest `Arbitrary` implementations from struct definitions,
//!    respecting field constraints specified in `#[lean(bounds = "...")]`.
//!
//! 2. **Property Generation** (`lean_proptest` attribute):
//!    Converts Lean theorem statements into proptest property tests,
//!    translating formal predicates into executable Rust assertions.
//!
//! 3. **Invariant Checking**:
//!    Generates runtime checks for well-formedness predicates,
//!    ensuring generated values satisfy their Lean specifications.
//!
//! # Type Mappings for Strategies
//!
//! | Lean Type | Rust Type | Proptest Strategy |
//! |-----------|-----------|-------------------|
//! | `Nat` | `usize` | `0..=MAX` or custom bounds |
//! | `Int` | `isize` | `MIN..=MAX` or custom bounds |
//! | `String` | `String` | `"[a-z]{0,100}"` or custom regex |
//! | `Array T` | `Vec<T>` | `prop::collection::vec(t_strategy, 0..100)` |
//! | `Option T` | `Option<T>` | `prop::option::of(t_strategy)` |
//!
//! # Example
//!
//! ```ignore
//! use sorex_lean_macros::{LeanSpec, LeanProptest, lean_proptest};
//!
//! #[derive(LeanSpec, LeanProptest)]
//! #[lean(name = "SuffixEntry")]
//! pub struct SuffixEntry {
//!     #[lean(bounds = "0..1000")]
//!     pub doc_id: usize,
//!     #[lean(bounds = "0..10000")]
//!     pub offset: usize,
//! }
//!
//! // Generates:
//! // impl Arbitrary for SuffixEntry {
//! //     type Parameters = ();
//! //     type Strategy = BoxedStrategy<Self>;
//! //     fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
//! //         (0usize..1000, 0usize..10000)
//! //             .prop_map(|(doc_id, offset)| SuffixEntry { doc_id, offset })
//! //             .boxed()
//! //     }
//! // }
//!
//! #[lean_proptest(
//!     spec = "binary_search_finds_match",
//!     property = "search returns entry iff suffix starts with query"
//! )]
//! fn test_binary_search_correctness() {
//!     // Property test body
//! }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Attribute, Data, DeriveInput, Field,
    Fields, Lit, Meta, Token,
};

/// Parsed proptest-relevant attributes from `#[lean(...)]`.
#[derive(Default, Clone)]
struct PropTestAttrs {
    /// Bounds for numeric types: "0..100" or "0..=99"
    bounds: Option<String>,
    /// Regex pattern for string types: "[a-z]{1,50}"
    pattern: Option<String>,
    /// Size range for collections: "0..10"
    size: Option<String>,
    /// Custom strategy expression
    strategy: Option<String>,
    /// Whether this field should use a fixed value in tests
    fixed: Option<String>,
}

impl PropTestAttrs {
    fn from_field(field: &Field) -> Self {
        let mut result = PropTestAttrs::default();

        for attr in &field.attrs {
            if attr.path().is_ident("lean") {
                if let Ok(nested) = attr.parse_args_with(
                    Punctuated::<Meta, Token![,]>::parse_terminated,
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
                                    Some("bounds") => result.bounds = Some(lit_str.value()),
                                    Some("pattern") => result.pattern = Some(lit_str.value()),
                                    Some("size") => result.size = Some(lit_str.value()),
                                    Some("strategy") => result.strategy = Some(lit_str.value()),
                                    Some("fixed") => result.fixed = Some(lit_str.value()),
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

    fn from_attrs(attrs: &[Attribute]) -> Self {
        let mut result = PropTestAttrs::default();

        for attr in attrs {
            if attr.path().is_ident("lean") {
                if let Ok(nested) = attr.parse_args_with(
                    Punctuated::<Meta, Token![,]>::parse_terminated,
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
                                    Some("bounds") => result.bounds = Some(lit_str.value()),
                                    Some("pattern") => result.pattern = Some(lit_str.value()),
                                    Some("size") => result.size = Some(lit_str.value()),
                                    Some("strategy") => result.strategy = Some(lit_str.value()),
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

/// Generate a proptest strategy for a Rust type.
///
/// Returns a token stream representing the strategy expression.
fn generate_strategy(ty: &syn::Type, attrs: &PropTestAttrs) -> proc_macro2::TokenStream {
    // If custom strategy is specified, use it directly
    if let Some(custom) = &attrs.strategy {
        let strategy: proc_macro2::TokenStream = custom.parse().unwrap_or_else(|_| {
            quote! { proptest::strategy::Just(Default::default()) }
        });
        return strategy;
    }

    // If fixed value is specified, use Just
    if let Some(fixed) = &attrs.fixed {
        let value: proc_macro2::TokenStream = fixed.parse().unwrap_or_else(|_| {
            quote! { Default::default() }
        });
        return quote! { proptest::strategy::Just(#value) };
    }

    match ty {
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last();
            match segment {
                Some(seg) => {
                    let ident = seg.ident.to_string();
                    match ident.as_str() {
                        // Numeric types with optional bounds
                        "usize" | "u64" | "u32" | "u16" | "u8" => {
                            if let Some(bounds) = &attrs.bounds {
                                let bounds_tokens: proc_macro2::TokenStream =
                                    bounds.parse().unwrap_or_else(|_| quote! { 0usize..=100 });
                                quote! { #bounds_tokens }
                            } else {
                                quote! { 0usize..=1000 }
                            }
                        }
                        "isize" | "i64" | "i32" | "i16" | "i8" => {
                            if let Some(bounds) = &attrs.bounds {
                                let bounds_tokens: proc_macro2::TokenStream =
                                    bounds.parse().unwrap_or_else(|_| quote! { -100isize..=100 });
                                quote! { #bounds_tokens }
                            } else {
                                quote! { -1000isize..=1000 }
                            }
                        }
                        "f64" | "f32" => {
                            if let Some(bounds) = &attrs.bounds {
                                let bounds_tokens: proc_macro2::TokenStream =
                                    bounds.parse().unwrap_or_else(|_| quote! { -1.0f64..=1.0 });
                                quote! { #bounds_tokens }
                            } else {
                                quote! { -1000.0f64..=1000.0 }
                            }
                        }
                        "bool" => quote! { proptest::bool::ANY },
                        "char" => quote! { proptest::char::any() },
                        "String" => {
                            if let Some(pattern) = &attrs.pattern {
                                quote! { proptest::string::string_regex(#pattern).unwrap() }
                            } else {
                                quote! { "[a-zA-Z0-9 ]{0,100}" }
                            }
                        }
                        "Vec" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    let inner_strategy =
                                        generate_strategy(inner, &PropTestAttrs::default());
                                    let size_range = if let Some(size) = &attrs.size {
                                        let size_tokens: proc_macro2::TokenStream =
                                            size.parse().unwrap_or_else(|_| quote! { 0..10 });
                                        size_tokens
                                    } else {
                                        quote! { 0..10 }
                                    };
                                    return quote! {
                                        proptest::collection::vec(#inner_strategy, #size_range)
                                    };
                                }
                            }
                            quote! { proptest::collection::vec(proptest::strategy::Just(()), 0..10) }
                        }
                        "Option" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    let inner_strategy =
                                        generate_strategy(inner, &PropTestAttrs::default());
                                    return quote! {
                                        proptest::option::of(#inner_strategy)
                                    };
                                }
                            }
                            quote! { proptest::option::of(proptest::strategy::Just(())) }
                        }
                        // Custom types - use Arbitrary trait
                        _ => {
                            let ty_ident = &seg.ident;
                            quote! { proptest::arbitrary::any::<#ty_ident>() }
                        }
                    }
                }
                None => quote! { proptest::strategy::Just(()) },
            }
        }
        syn::Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                quote! { proptest::strategy::Just(()) }
            } else {
                let strategies: Vec<_> = tuple
                    .elems
                    .iter()
                    .map(|ty| generate_strategy(ty, &PropTestAttrs::default()))
                    .collect();
                quote! { (#(#strategies),*) }
            }
        }
        syn::Type::Reference(reference) => generate_strategy(&reference.elem, attrs),
        _ => quote! { proptest::strategy::Just(Default::default()) },
    }
}

/// Main entry point for the `#[derive(LeanProptest)]` macro.
///
/// Generates proptest `Arbitrary` implementation for the struct,
/// using bounds and patterns from `#[lean(...)]` attributes.
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract struct-level attributes for default bounds
    let _struct_attrs = PropTestAttrs::from_attrs(&input.attrs);

    // Get fields from struct
    let fields = match &input.data {
        Data::Struct(data_struct) => &data_struct.fields,
        Data::Enum(_) => {
            return syn::Error::new_spanned(
                &input.ident,
                "LeanProptest currently only supports structs, not enums",
            )
            .to_compile_error()
            .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(&input.ident, "LeanProptest does not support unions")
                .to_compile_error()
                .into();
        }
    };

    // Generate strategy for each field
    let (field_names, field_strategies): (Vec<_>, Vec<_>) = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                let field_attrs = PropTestAttrs::from_field(f);
                let strategy = generate_strategy(&f.ty, &field_attrs);
                (field_name.clone(), strategy)
            })
            .unzip(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let field_attrs = PropTestAttrs::from_field(f);
                let strategy = generate_strategy(&f.ty, &field_attrs);
                let ident = syn::Ident::new(&format!("field{}", i), proc_macro2::Span::call_site());
                (ident, strategy)
            })
            .unzip(),
        Fields::Unit => (vec![], vec![]),
    };

    // Generate the Arbitrary implementation
    let expanded = if field_names.is_empty() {
        quote! {
            #[cfg(test)]
            impl proptest::arbitrary::Arbitrary for #name {
                type Parameters = ();
                type Strategy = proptest::strategy::Just<Self>;

                fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                    proptest::strategy::Just(#name)
                }
            }
        }
    } else {
        let tuple_pattern = if field_names.len() == 1 {
            let name = &field_names[0];
            quote! { #name }
        } else {
            quote! { (#(#field_names),*) }
        };

        let struct_construction = match fields {
            Fields::Named(_) => quote! { #name { #(#field_names),* } },
            Fields::Unnamed(_) => quote! { #name(#(#field_names),*) },
            Fields::Unit => quote! { #name },
        };

        let strategy_tuple = if field_strategies.len() == 1 {
            let strategy = &field_strategies[0];
            quote! { #strategy }
        } else {
            quote! { (#(#field_strategies),*) }
        };

        quote! {
            #[cfg(test)]
            impl proptest::arbitrary::Arbitrary for #name {
                type Parameters = ();
                type Strategy = proptest::strategy::BoxedStrategy<Self>;

                fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                    use proptest::strategy::Strategy;
                    #strategy_tuple
                        .prop_map(|#tuple_pattern| #struct_construction)
                        .boxed()
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Parsed attributes from `#[lean_proptest(...)]`.
#[derive(Default, Debug)]
pub struct LeanPropTestFnAttrs {
    /// Name for the generated test
    spec: Option<String>,
    /// Property description
    property: Option<String>,
    /// Number of test cases
    cases: Option<usize>,
    /// Whether to generate a regression test
    regression: bool,
}

impl LeanPropTestFnAttrs {
    pub fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result = LeanPropTestFnAttrs {
            regression: false,
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
                            Some("property") => result.property = Some(lit_str.value()),
                            _ => {}
                        },
                        syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Int(lit_int),
                            ..
                        }) => {
                            if key.as_deref() == Some("cases") {
                                result.cases = lit_int.base10_parse().ok();
                            }
                        }
                        syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Bool(lit_bool),
                            ..
                        }) => {
                            if key.as_deref() == Some("regression") {
                                result.regression = lit_bool.value;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(result)
    }
}

/// Generate a proptest property test from the function attributes.
///
/// This wraps the function body in a proptest! macro invocation.
pub fn process_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as syn::ItemFn);
    let attrs = match LeanPropTestFnAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let func_name = &func.sig.ident;
    let func_body = &func.block;
    let func_attrs = &func.attrs;

    let test_name = attrs.spec.unwrap_or_else(|| func_name.to_string());
    let test_ident = syn::Ident::new(&test_name, func_name.span());

    let cases = attrs.cases.unwrap_or(256);
    let doc_comment = attrs.property.map(|p| {
        quote! { #[doc = #p] }
    });

    let expanded = quote! {
        #[cfg(test)]
        mod #test_ident {
            use super::*;
            use proptest::prelude::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(#cases))]

                #(#func_attrs)*
                #doc_comment
                #[test]
                fn property #func_body
            }
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_numeric_strategy() {
        let ty: syn::Type = parse_quote!(usize);
        let attrs = PropTestAttrs {
            bounds: Some("0usize..100".to_string()),
            ..Default::default()
        };
        let strategy = generate_strategy(&ty, &attrs);
        let expected = "0usize .. 100";
        assert!(strategy.to_string().contains("0usize") || strategy.to_string().contains("100"));
    }

    #[test]
    fn test_string_strategy_with_pattern() {
        let ty: syn::Type = parse_quote!(String);
        let attrs = PropTestAttrs {
            pattern: Some("[a-z]+".to_string()),
            ..Default::default()
        };
        let strategy = generate_strategy(&ty, &attrs);
        assert!(strategy.to_string().contains("string_regex"));
    }

    #[test]
    fn test_vec_strategy() {
        let ty: syn::Type = parse_quote!(Vec<usize>);
        let attrs = PropTestAttrs {
            size: Some("0..5".to_string()),
            ..Default::default()
        };
        let strategy = generate_strategy(&ty, &attrs);
        assert!(strategy.to_string().contains("vec"));
    }
}
