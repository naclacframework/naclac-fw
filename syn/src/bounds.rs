//! The maximum serialized size, and the length-limit check, of a field of a
//! Borsh or Wincode `#[component]` or `#[defined_type]`.
//!
//! A type's name is read only to find the structure that `#[max_len]` limits:
//! `String`, `Vec<_>` and `Option<_>`. Every other type is resolved by the
//! compiler through `SerializedBounds`, so a type alias gives the same answer
//! as the type it names, and a type with no known maximum size is a compile
//! error rather than a guess. A `String` or `Vec` with no limit left to apply
//! is reported as unbounded.

use {
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{punctuated::Punctuated, Attribute, GenericArgument, LitInt, PathArguments, Token, Type},
};

/// A field's maximum serialized size, as a `usize` expression, and the check
/// that its `Vec` and `String` values are within their limits, as a `bool`
/// expression.
pub struct Bound {
    pub size: TokenStream,
    pub check: TokenStream,
}

/// Why a field has no [`Bound`].
pub enum Problem {
    /// A `Vec` or `String` has no `#[max_len]` limit.
    Unbounded,
    /// The `#[max_len]` limits do not fit the field.
    Invalid(syn::Error),
}

enum Shape<'a> {
    String,
    Vec(&'a Type),
    Option(&'a Type),
    Other,
}

/// The limits of a field's `#[max_len(a, b, ..)]`, outermost first, or none
/// when it has no such attribute.
pub fn max_len_values(attrs: &[Attribute]) -> syn::Result<Vec<LitInt>> {
    let mut found: Option<Vec<LitInt>> = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("max_len")) {
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "Naclac Error: a field takes one `#[max_len(..)]`; list every limit in it, \
                 outermost first, as in `#[max_len(10, 32)]`",
            ));
        }
        let values = attr.parse_args_with(Punctuated::<LitInt, Token![,]>::parse_terminated)?;
        found = Some(values.into_iter().collect());
    }
    Ok(found.unwrap_or_default())
}

fn shape(ty: &Type) -> Shape<'_> {
    let Type::Path(path) = ty else {
        return Shape::Other;
    };
    if path.qself.is_some() {
        return Shape::Other;
    }
    let Some(segment) = path.path.segments.last() else {
        return Shape::Other;
    };
    let inner = || match &segment.arguments {
        PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().find_map(|argument| match argument {
                GenericArgument::Type(inner) => Some(inner),
                _ => None,
            })
        }
        _ => None,
    };
    if segment.ident == "String" {
        Shape::String
    } else if segment.ident == "Vec" {
        inner().map_or(Shape::Other, Shape::Vec)
    } else if segment.ident == "Option" {
        inner().map_or(Shape::Other, Shape::Option)
    } else {
        Shape::Other
    }
}

fn next_limit<'a>(limits: &'a [LitInt], used: &mut usize) -> Result<&'a LitInt, Problem> {
    let limit = limits.get(*used).ok_or(Problem::Unbounded)?;
    *used += 1;
    Ok(limit)
}

fn bound(
    ty: &Type,
    limits: &[LitInt],
    used: &mut usize,
    value: &TokenStream,
    depth: usize,
    core: &syn::Path,
) -> Result<Bound, Problem> {
    match shape(ty) {
        Shape::String => {
            let limit = next_limit(limits, used)?;
            Ok(Bound {
                size: quote! { (4usize + #limit) },
                check: quote! { ((#value).len() <= #limit) },
            })
        }
        Shape::Vec(element) => {
            let limit = next_limit(limits, used)?;
            let item = format_ident!("__item{}", depth);
            let inner = bound(element, limits, used, &quote! { #item }, depth + 1, core)?;
            let (inner_size, inner_check) = (inner.size, inner.check);
            Ok(Bound {
                size: quote! { (4usize + #limit * #inner_size) },
                check: quote! {
                    ((#value).len() <= #limit && (#value).iter().all(|#item| #inner_check))
                },
            })
        }
        Shape::Option(element) => {
            let item = format_ident!("__item{}", depth);
            let inner = bound(element, limits, used, &quote! { #item }, depth + 1, core)?;
            let (inner_size, inner_check) = (inner.size, inner.check);
            Ok(Bound {
                size: quote! { (1usize + #inner_size) },
                check: quote! {
                    match #value {
                        Some(#item) => #inner_check,
                        None => true,
                    }
                },
            })
        }
        Shape::Other => Ok(Bound {
            size: quote! { <#ty as #core::SerializedBounds>::MAX_SIZE },
            check: quote! { #core::SerializedBounds::within_bounds(#value) },
        }),
    }
}

/// The [`Bound`] of a field of type `ty` with the `#[max_len]` limits
/// `limits`. `value` is an expression of type `&ty` naming the field's value,
/// and `core` is the path that exports `SerializedBounds`.
pub fn field_bound(
    ty: &Type,
    limits: &[LitInt],
    value: &TokenStream,
    core: &syn::Path,
) -> Result<Bound, Problem> {
    let mut used = 0usize;
    let result = bound(ty, limits, &mut used, value, 0, core)?;
    if let Some(unused) = limits.get(used) {
        return Err(Problem::Invalid(syn::Error::new_spanned(
            unused,
            "Naclac Error: this `#[max_len]` has more limits than the field has `Vec` or \
             `String` levels to apply them to",
        )));
    }
    Ok(result)
}
