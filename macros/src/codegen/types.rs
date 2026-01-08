//! Type translation from Rust to Lean 4.
//!
//! This module provides utilities for converting Rust type syntax
//! into equivalent Lean 4 type expressions.

use syn::Type;

/// Convert a Rust type to its Lean 4 equivalent.
///
/// # Type Mappings
///
/// | Rust Type | Lean Type |
/// |-----------|-----------|
/// | `usize`, `u64`, `u32`, `u16`, `u8` | `Nat` |
/// | `isize`, `i64`, `i32`, `i16`, `i8` | `Int` |
/// | `f64`, `f32` | `Float` |
/// | `bool` | `Bool` |
/// | `char` | `Char` |
/// | `String`, `&str` | `String` |
/// | `Vec<T>` | `Array T` |
/// | `Option<T>` | `Option T` |
/// | `(A, B)` | `A × B` |
/// | `Box<T>` | `T` |
/// | `&T`, `&mut T` | `T` |
///
/// # Examples
///
/// ```ignore
/// use syn::parse_quote;
/// use sorex_lean_macros::codegen::rust_type_to_lean;
///
/// let ty: syn::Type = parse_quote!(Vec<String>);
/// assert_eq!(rust_type_to_lean(&ty), "Array String");
/// ```
pub fn rust_type_to_lean(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last();
            match segment {
                Some(seg) => {
                    let ident = seg.ident.to_string();
                    match ident.as_str() {
                        // Unsigned integers -> Nat
                        "usize" | "u64" | "u32" | "u16" | "u8" => "Nat".to_string(),

                        // Signed integers -> Int
                        "isize" | "i64" | "i32" | "i16" | "i8" => "Int".to_string(),

                        // Floating point -> Float
                        "f64" | "f32" => "Float".to_string(),

                        // Primitives
                        "bool" => "Bool".to_string(),
                        "char" => "Char".to_string(),

                        // Strings
                        "String" | "str" => "String".to_string(),

                        // Vec -> Array
                        "Vec" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    let inner_lean = rust_type_to_lean(inner);
                                    return format!("Array {}", parenthesize_if_needed(&inner_lean));
                                }
                            }
                            "Array _".to_string()
                        }

                        // Option -> Option
                        "Option" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    let inner_lean = rust_type_to_lean(inner);
                                    return format!(
                                        "Option {}",
                                        parenthesize_if_needed(&inner_lean)
                                    );
                                }
                            }
                            "Option _".to_string()
                        }

                        // Result -> Except
                        "Result" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                let mut iter = args.args.iter();
                                if let (
                                    Some(syn::GenericArgument::Type(ok_ty)),
                                    Some(syn::GenericArgument::Type(err_ty)),
                                ) = (iter.next(), iter.next())
                                {
                                    let ok_lean = rust_type_to_lean(ok_ty);
                                    let err_lean = rust_type_to_lean(err_ty);
                                    return format!(
                                        "Except {} {}",
                                        parenthesize_if_needed(&err_lean),
                                        parenthesize_if_needed(&ok_lean)
                                    );
                                }
                            }
                            "Except _ _".to_string()
                        }

                        // HashMap -> Std.HashMap (from Lean std4)
                        "HashMap" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                let mut iter = args.args.iter();
                                if let (
                                    Some(syn::GenericArgument::Type(key_ty)),
                                    Some(syn::GenericArgument::Type(val_ty)),
                                ) = (iter.next(), iter.next())
                                {
                                    let key_lean = rust_type_to_lean(key_ty);
                                    let val_lean = rust_type_to_lean(val_ty);
                                    return format!(
                                        "Std.HashMap {} {}",
                                        parenthesize_if_needed(&key_lean),
                                        parenthesize_if_needed(&val_lean)
                                    );
                                }
                            }
                            "Std.HashMap _ _".to_string()
                        }

                        // Box/Rc/Arc -> unwrap the inner type
                        "Box" | "Rc" | "Arc" => {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                    return rust_type_to_lean(inner);
                                }
                            }
                            "_".to_string()
                        }

                        // Unit type
                        "()" => "Unit".to_string(),

                        // Custom types - preserve the name
                        other => other.to_string(),
                    }
                }
                None => "_".to_string(),
            }
        }

        // Tuple types -> Product types (×)
        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                "Unit".to_string()
            } else {
                let parts: Vec<String> = tuple.elems.iter().map(rust_type_to_lean).collect();
                parts.join(" × ")
            }
        }

        // Reference types -> strip the reference
        Type::Reference(reference) => rust_type_to_lean(&reference.elem),

        // Slice types -> Array
        Type::Slice(slice) => {
            let inner_lean = rust_type_to_lean(&slice.elem);
            format!("Array {}", parenthesize_if_needed(&inner_lean))
        }

        // Array types [T; N] -> Array T (we lose the size information)
        Type::Array(array) => {
            let inner_lean = rust_type_to_lean(&array.elem);
            format!("Array {}", parenthesize_if_needed(&inner_lean))
        }

        // Pointer types -> strip the pointer
        Type::Ptr(ptr) => rust_type_to_lean(&ptr.elem),

        // Function pointers -> simplified representation
        Type::BareFn(_) => "(_ → _)".to_string(),

        // Impl trait -> placeholder
        Type::ImplTrait(_) => "_".to_string(),

        // Other types we don't handle -> placeholder
        _ => "_".to_string(),
    }
}

/// Parenthesize a Lean type if it contains spaces (is a type application).
fn parenthesize_if_needed(lean_type: &str) -> String {
    if lean_type.contains(' ') && !lean_type.starts_with('(') {
        format!("({})", lean_type)
    } else {
        lean_type.to_string()
    }
}

/// Convert a Rust identifier to a valid Lean identifier.
///
/// Lean identifiers follow similar rules to Rust but with some differences:
/// - Reserved words need to be escaped with guillemets: `«end»`
/// - Snake_case is valid in Lean but camelCase is conventional
pub fn rust_ident_to_lean(ident: &str) -> String {
    // Lean 4 reserved words that conflict with common Rust field names
    const LEAN_RESERVED: &[&str] = &[
        "end", "where", "do", "if", "then", "else", "match", "with", "fun", "let", "in", "have",
        "show", "from", "by", "at", "this", "type", "class", "instance", "structure", "inductive",
        "def", "theorem", "lemma", "example", "axiom", "constant", "variable", "universe",
        "namespace", "section", "open", "import", "export", "protected", "private", "partial",
        "unsafe", "noncomputable", "mutual", "notation", "macro", "syntax", "elab", "deriving",
    ];

    if LEAN_RESERVED.contains(&ident) {
        format!("«{}»", ident)
    } else {
        ident.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_primitive_types() {
        let ty: Type = parse_quote!(usize);
        assert_eq!(rust_type_to_lean(&ty), "Nat");

        let ty: Type = parse_quote!(i64);
        assert_eq!(rust_type_to_lean(&ty), "Int");

        let ty: Type = parse_quote!(f64);
        assert_eq!(rust_type_to_lean(&ty), "Float");

        let ty: Type = parse_quote!(bool);
        assert_eq!(rust_type_to_lean(&ty), "Bool");
    }

    #[test]
    fn test_string_types() {
        let ty: Type = parse_quote!(String);
        assert_eq!(rust_type_to_lean(&ty), "String");

        let ty: Type = parse_quote!(&str);
        assert_eq!(rust_type_to_lean(&ty), "String");
    }

    #[test]
    fn test_vec_type() {
        let ty: Type = parse_quote!(Vec<String>);
        assert_eq!(rust_type_to_lean(&ty), "Array String");

        let ty: Type = parse_quote!(Vec<Vec<usize>>);
        assert_eq!(rust_type_to_lean(&ty), "Array (Array Nat)");
    }

    #[test]
    fn test_option_type() {
        let ty: Type = parse_quote!(Option<String>);
        assert_eq!(rust_type_to_lean(&ty), "Option String");

        let ty: Type = parse_quote!(Option<Vec<usize>>);
        assert_eq!(rust_type_to_lean(&ty), "Option (Array Nat)");
    }

    #[test]
    fn test_tuple_type() {
        let ty: Type = parse_quote!((usize, String));
        assert_eq!(rust_type_to_lean(&ty), "Nat × String");

        let ty: Type = parse_quote!((usize, String, bool));
        assert_eq!(rust_type_to_lean(&ty), "Nat × String × Bool");
    }

    #[test]
    fn test_reference_stripped() {
        let ty: Type = parse_quote!(&String);
        assert_eq!(rust_type_to_lean(&ty), "String");

        let ty: Type = parse_quote!(&mut Vec<usize>);
        assert_eq!(rust_type_to_lean(&ty), "Array Nat");
    }

    #[test]
    fn test_custom_type() {
        let ty: Type = parse_quote!(SuffixEntry);
        assert_eq!(rust_type_to_lean(&ty), "SuffixEntry");

        let ty: Type = parse_quote!(SearchIndex);
        assert_eq!(rust_type_to_lean(&ty), "SearchIndex");
    }

    #[test]
    fn test_reserved_word_escaping() {
        assert_eq!(rust_ident_to_lean("end"), "«end»");
        assert_eq!(rust_ident_to_lean("where"), "«where»");
        assert_eq!(rust_ident_to_lean("doc_id"), "doc_id");
    }
}
