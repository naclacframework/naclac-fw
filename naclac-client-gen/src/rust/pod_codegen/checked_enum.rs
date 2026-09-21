//! Hand-rolled `bytemuck::CheckedBitPattern` generation for a `#[repr(Int)]`
//! enum, used by `defined_type`'s zero-copy enum branch.
//!
//! This is a from-scratch port of `bytemuck_derive`'s own algorithm for the
//! `Repr::Integer` case (verified against `bytemuck_derive-1.12.0`'s real
//! source, `generate_checked_bit_pattern_enum_with_fields`/
//! `generate_checked_bit_pattern_struct`/`VariantDiscriminantIterator`) —
//! not a simplified reinvention. Every generated path is fully qualified
//! through `naclac_lang::prelude::bytemuck`, and every trait impl here is
//! hand-written rather than produced by invoking `bytemuck`'s own derive
//! macros — that's specifically what avoids `bytemuck`'s derive needing
//! `bytemuck` as a direct dependency of the invoking crate (a hand-written
//! `unsafe impl` can reference any qualified path; it's bytemuck_derive's
//! own internally-generated types that can't be redirected via
//! `#[bytemuck(crate = "...")]`, since that override doesn't propagate to
//! them — see docs/plan/anchor-idl-conversion-and-enum-pod-audit.md).
//!
//! Field types are handled fully generically via `<FieldTy as
//! bytemuck::CheckedBitPattern>::Bits`/`is_valid_bit_pattern` — not
//! hardcoded to naclac's own known primitive set — so a variant field that
//! is itself another `#[defined_type]` enum recurses correctly through
//! Rust's own trait resolution, exactly like real `bytemuck_derive` does
//! for arbitrarily nested `CheckedBitPattern` types.
//!
//! **This file is a deliberate, verbatim duplicate of
//! `naclac-macros/src/checked_enum.rs`** (see
//! `docs/plan/anchor-idl-conversion-and-enum-pod-audit.md`'s Gap #2 section
//! for why: `naclac-macros` is a `proc-macro = true` crate, so
//! `naclac-client-gen` cannot depend on it to call this code directly, and
//! moving it into a shared crate or into `naclac-core` was rejected —
//! `naclac-core` already depends on `naclac-macros`, so the reverse would
//! be a circular crate dependency; a brand-new crate was rejected in favor
//! of this simpler direct copy). The only intentional difference from the
//! original is that the `bytemuck` crate path is a parameter here
//! (`crate_path`) instead of the hardcoded `naclac_lang::prelude` literal,
//! since a generated client crate has no `naclac_lang` dependency and
//! reaches `bytemuck` through `crate::sdk_core_offchain`/
//! `crate::sdk_core_cpi` instead. If the on-chain algorithm in
//! `naclac-macros/src/checked_enum.rs` ever changes, this file must be
//! updated to match by hand — nothing enforces that automatically.

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Fields, Variant};

/// Computes each variant's real discriminant value, mirroring Rust's own
/// enum discriminant rule exactly (same algorithm as bytemuck_derive's
/// `VariantDiscriminantIterator`): starts at -1; an explicit `= N` sets the
/// running value to `N`; otherwise the running value increments by 1 from
/// the previous variant. Getting this wrong for an enum with an explicit
/// discriminant would silently validate the wrong tag byte.
fn variant_discriminants(variants: &[&Variant]) -> Vec<i128> {
    let mut last_value: i128 = -1;
    variants
        .iter()
        .map(|v| {
            if let Some((_, expr)) = &v.discriminant {
                last_value = parse_int_expr(expr);
            } else {
                last_value = last_value.wrapping_add(1);
            }
            last_value
        })
        .collect()
}

fn parse_int_expr(expr: &syn::Expr) -> i128 {
    match expr {
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => -parse_int_expr(expr),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => int
            .base10_parse()
            .expect("defined_type: enum discriminant literal must fit in i128"),
        _ => panic!("defined_type: enum discriminant must be a literal integer expression"),
    }
}

/// Generates the extra items (per-variant payload/Bits structs, the union)
/// plus the `impl bytemuck::CheckedBitPattern for #ident` body, for a
/// `#[repr(#tag_ty)]` enum. Returns `(extra_items, checked_bit_pattern_impl)`.
///
/// `crate_path` is the path prefix under which `bytemuck` is reachable from
/// the generated code's own crate root (e.g. `"naclac_lang::prelude"` for
/// on-chain code, `"crate::sdk_core_offchain"`/`"crate::sdk_core_cpi"` for a
/// generated client crate) — every `bytemuck` reference below is qualified
/// through it instead of a hardcoded literal, the one intentional deviation
/// from `naclac-macros/src/checked_enum.rs` (see module doc comment).
pub fn generate(
    ident: &Ident,
    vis: &syn::Visibility,
    variants: &syn::punctuated::Punctuated<Variant, syn::Token![,]>,
    tag_ty: &syn::Type,
    crate_path: &str,
) -> (TokenStream, TokenStream) {
    let bytemuck: syn::Path = syn::parse_str(&format!("{}::bytemuck", crate_path))
        .expect("defined_type: crate_path must be a valid Rust path");

    let variants: Vec<&Variant> = variants.iter().collect();
    let discriminants = variant_discriminants(&variants);

    let mut extra_items = Vec::new();
    let mut union_fields = Vec::new();
    let mut match_arms = Vec::new();

    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_ident = &variant.ident;
        let discriminant_lit =
            syn::LitInt::new(&discriminant.to_string(), variant_ident.span());

        if matches!(&variant.fields, Fields::Unit) {
            // A unit variant's entire representation is the tag — nothing
            // else to validate once the tag itself matches. Genuinely no
            // union member needed (matches bytemuck_derive's own fieldless
            // handling), not a corner cut: there are no other bytes.
            match_arms.push(quote! { #discriminant_lit => true, });
            continue;
        }

        let payload_ident = format_ident!("{}Variant{}", ident, variant_ident);
        let bits_ident = format_ident!("{}Bits", payload_ident);
        let field_tys: Vec<&syn::Type> = variant.fields.iter().map(|f| &f.ty).collect();

        // Payload struct: tag as an explicit, literal first field — this is
        // the actual mechanism that makes the tag's offset well-defined
        // without needing #[repr(C, Int)] on the original enum (a #[repr(C)]
        // *struct*'s first field is unambiguously at offset 0; that guarantee
        // is what's being borrowed here, not anything about the enum's own
        // repr). See bytemuck_derive's own comment citing
        // https://doc.rust-lang.org/reference/type-layout.html#primitive-representation-of-enums-with-fields.
        extra_items.push(quote! {
            #[repr(C)]
            #[derive(Clone, Copy)]
            #[allow(non_snake_case)]
            #vis struct #payload_ident(#tag_ty, #(#field_tys),*);
        });

        // Bits struct: every field replaced by its own `CheckedBitPattern::Bits`
        // — recursive and generic, so a nested `#[defined_type]` enum field
        // resolves through its own hand-rolled impl automatically.
        extra_items.push(quote! {
            #[repr(C)]
            #[derive(Clone, Copy)]
            #[allow(non_snake_case)]
            #vis struct #bits_ident(
                <#tag_ty as #bytemuck::CheckedBitPattern>::Bits,
                #(<#field_tys as #bytemuck::CheckedBitPattern>::Bits),*
            );
            // SAFETY: every field is itself `AnyBitPattern` (guaranteed by
            // `CheckedBitPattern::Bits: AnyBitPattern`), and #[repr(C)]
            // composition of AnyBitPattern fields is itself AnyBitPattern —
            // padding bytes (if any) are not a concern for AnyBitPattern,
            // only for Pod/NoUninit (the mutable-cast traits, not used here).
            unsafe impl #bytemuck::AnyBitPattern for #bits_ident {}
            // SAFETY: `AnyBitPattern: Zeroable` — an all-zero byte pattern is
            // valid for every field here for the same reason AnyBitPattern
            // itself holds (every field's own Bits type is AnyBitPattern,
            // hence Zeroable).
            unsafe impl #bytemuck::Zeroable for #bits_ident {}
        });

        let field_indices: Vec<syn::Index> =
            (1..=field_tys.len()).map(syn::Index::from).collect();
        extra_items.push(quote! {
            unsafe impl #bytemuck::CheckedBitPattern for #payload_ident {
                type Bits = #bits_ident;
                #[inline]
                fn is_valid_bit_pattern(bits: &Self::Bits) -> bool {
                    <#tag_ty as #bytemuck::CheckedBitPattern>::is_valid_bit_pattern(&bits.0)
                    #(&& <#field_tys as #bytemuck::CheckedBitPattern>::is_valid_bit_pattern(&bits.#field_indices))*
                }
            }
        });

        union_fields.push(quote! { #variant_ident: #bits_ident });
        match_arms.push(quote! {
            #discriminant_lit => unsafe {
                <#payload_ident as #bytemuck::CheckedBitPattern>::is_valid_bit_pattern(&bits.#variant_ident)
            },
        });
    }

    let bits_ident = format_ident!("{}Bits", ident);
    extra_items.push(quote! {
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(non_snake_case)]
        #vis union #bits_ident {
            __tag: #tag_ty,
            #(#union_fields),*
        }
        // SAFETY: every member is itself AnyBitPattern (see field-level
        // safety comments above); a #[repr(C)] union of AnyBitPattern
        // members is itself AnyBitPattern regardless of which member's
        // bytes are semantically "active" — any member can be read.
        unsafe impl #bytemuck::AnyBitPattern for #bits_ident {}
        // SAFETY: `AnyBitPattern: Zeroable` — same reasoning as the impl above.
        unsafe impl #bytemuck::Zeroable for #bits_ident {}
    });

    let checked_bit_pattern_impl = quote! {
        unsafe impl #bytemuck::CheckedBitPattern for #ident {
            type Bits = #bits_ident;
            #[inline]
            fn is_valid_bit_pattern(bits: &Self::Bits) -> bool {
                match unsafe { bits.__tag } {
                    #(#match_arms)*
                    _ => false,
                }
            }
        }
    };

    (
        quote! { #(#extra_items)* },
        checked_bit_pattern_impl,
    )
}
