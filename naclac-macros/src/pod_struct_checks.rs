//! Hand-rolled compile-time Pod-safety machinery for a struct — used by
//! both `defined_type`'s struct branch and `#[component]`'s zero-copy
//! branch, so the same logic can't drift between the two. Handles both
//! named-field (`struct Foo { x: T }`) and tuple (`struct Foo(T)`) structs.
//!
//! A proc macro can't compute field sizes/alignments itself (it runs on
//! pure syntax, before type resolution), but it can emit `const` expressions
//! using `core::mem::size_of`/`align_of` for the compiler to resolve *after*
//! macro expansion, once every field's real type layout is known — the same
//! technique `event.rs` (`naclac-macros/src/event.rs:131-176`) already uses
//! for `#[event]` zero-copy structs.
//!
//! **Named-field structs** get a padding field auto-inserted after *every*
//! field (sized 0 when none is needed), each computed this way from the
//! cumulative offset of the fields before it and the alignment the next
//! field (or, after the last field, the struct's own overall alignment)
//! requires — closing every internal gap exactly, not just the trailing one,
//! with no field reordering (fields are always accessed by name, so their
//! declaration order relative to each other never changes) and no
//! hand-typed padding size for the developer to get wrong. A closing
//! self-check assertion (`size_of::<Self>() == size_sum_with_computed_gaps`)
//! guards against a bug in this computation itself, not against a user
//! mistake — with correctly-computed gaps it holds by construction.
//!
//! **Tuple structs** keep the original trailing-only behavior instead:
//! inserting padding *between* positional fields would shift every
//! subsequent field's index (`.0`, `.1`, ...), silently breaking any
//! existing positional access — a hazard named-field structs don't have.
//! Reordering is still not attempted for either shape, for the reason the
//! original version of this file documented: ranking an arbitrary custom
//! field type's alignment isn't something macro-time syntax can safely
//! guess, and choosing between two different field orderings isn't
//! expressible via a deferred `const` the way sizing a fixed padding slot is.

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Fields;

fn align_max_expr(field_tys: &[&syn::Type]) -> TokenStream {
    if field_tys.is_empty() {
        quote! { 1usize }
    } else {
        quote! {{
            let mut __a = 1usize;
            #(
                let __b = ::core::mem::align_of::<#field_tys>();
                if __b > __a { __a = __b; }
            )*
            __a
        }}
    }
}

/// Returns `(padded_fields, extra_items)`:
/// - `padded_fields`: the original fields, plus auto-sized padding —
///   interleaved after every field for a named-field struct, or a single
///   trailing field for a tuple struct or an empty field list. Brace- or
///   paren-delimited to match exactly what `syn::FieldsNamed`/
///   `syn::FieldsUnnamed` themselves parse (no trailing semicolon either
///   way — a tuple struct's `;` is a separate token on the struct item
///   itself, not part of its `Fields`).
/// - `extra_items`: the padding-size `const`(s), the no-internal-gap
///   assertion, and the per-field `Pod`-bound assertion — emit these
///   alongside the struct, not inside it.
///
/// `cfg` is prepended to every individual item `extra_items` emits (empty
/// for unconditional) — a caller that needs its whole output cfg-gated
/// (e.g. `#[component]`'s zero-copy branch, chosen per-item by the calling
/// crate's own `#[cfg(...)]` rather than by this macro) can't just wrap the
/// combined `extra_items` stream in one outer attribute, since that would
/// only apply to the first of several sibling top-level items.
pub fn generate(ident: &Ident, fields: &Fields, cfg: &TokenStream) -> (TokenStream, TokenStream) {
    let is_tuple = matches!(fields, Fields::Unnamed(_));
    let field_vis: Vec<&syn::Visibility> = fields.iter().map(|f| &f.vis).collect();
    let field_tys: Vec<&syn::Type> = fields.iter().map(|f| &f.ty).collect();
    let n = field_tys.len();

    if is_tuple || n == 0 {
        return generate_trailing_only(ident, is_tuple, &field_vis, &field_tys, cfg);
    }

    let field_idents: Vec<&syn::Ident> = fields
        .iter()
        .map(|f| f.ident.as_ref().expect("named-field branch: checked by is_tuple"))
        .collect();

    let gap_field_idents: Vec<Ident> =
        (0..n).map(|i| format_ident!("__naclac_padding_{}", i)).collect();
    if let Some(collision) = field_idents
        .iter()
        .find(|id| gap_field_idents.iter().any(|g| **id == g))
    {
        let err = syn::Error::new(
            collision.span(),
            "Naclac Error: field names matching `__naclac_padding_<N>` are reserved for \
             defined_type/#[component]'s own auto-inserted internal padding — rename this field.",
        )
        .to_compile_error();
        let unpadded_fields = quote! {
            { #(#field_vis #field_idents: #field_tys,)* }
        };
        return (unpadded_fields, quote! { #cfg #err });
    }

    let align_max = align_max_expr(&field_tys);
    let gap_const_idents: Vec<Ident> =
        (0..n).map(|i| format_ident!("__{}_GAP_{}", ident, i)).collect();

    let mut gap_consts = Vec::with_capacity(n);
    for i in 0..n {
        let ty_i = field_tys[i];
        let mut offset_terms: Vec<TokenStream> = Vec::with_capacity(2 * i + 1);
        for k in 0..i {
            let ty_k = field_tys[k];
            let gap_k = &gap_const_idents[k];
            offset_terms.push(quote! { ::core::mem::size_of::<#ty_k>() });
            offset_terms.push(quote! { #gap_k });
        }
        offset_terms.push(quote! { ::core::mem::size_of::<#ty_i>() });
        let offset_expr = offset_terms
            .into_iter()
            .reduce(|acc, term| quote! { #acc + #term })
            .expect("offset_terms always has the final size_of push");

        let align_expr = if i + 1 < n {
            let ty_next = field_tys[i + 1];
            quote! { ::core::mem::align_of::<#ty_next>() }
        } else {
            align_max.clone()
        };

        let gap_i = &gap_const_idents[i];
        gap_consts.push(quote! {
            #cfg
            #[allow(non_upper_case_globals)]
            const #gap_i: usize = naclac_lang::prelude::compute_padding_gap(#offset_expr, #align_expr);
        });
    }

    let padded_fields = quote! {
        {
            #(
                #field_vis #field_idents: #field_tys,
                pub #gap_field_idents: [u8; #gap_const_idents],
            )*
        }
    };

    let total_size_expr = {
        let mut terms: Vec<TokenStream> = Vec::with_capacity(2 * n);
        for i in 0..n {
            let ty_i = field_tys[i];
            let gap_i = &gap_const_idents[i];
            terms.push(quote! { ::core::mem::size_of::<#ty_i>() });
            terms.push(quote! { #gap_i });
        }
        terms
            .into_iter()
            .reduce(|acc, term| quote! { #acc + #term })
            .expect("n > 0 in this branch (n == 0 handled by generate_trailing_only)")
    };

    let self_check_message = format!(
        "defined_type/#[component]: `{}`'s auto-computed internal padding doesn't match the \
         real compiler layout — this indicates a bug in naclac's own padding computation \
         (naclac-macros/src/pod_struct_checks.rs), not a field ordering issue for you to fix",
        ident
    );

    let extra_items = quote! {
        #(#gap_consts)*

        #cfg
        const _: () = {
            assert!(
                ::core::mem::size_of::<#ident>() == (#total_size_expr),
                #self_check_message,
            );
        };

        #cfg
        const _: fn() = || {
            fn assert_impl<T: naclac_lang::prelude::Pod>() {}
            #(assert_impl::<#field_tys>();)*
        };
    };

    (padded_fields, extra_items)
}

/// Original trailing-only behavior: tuple structs (padding can only be
/// appended, not inserted, without shifting positional indices) and empty
/// field lists (nothing to pad between).
fn generate_trailing_only(
    ident: &Ident,
    is_tuple: bool,
    field_vis: &[&syn::Visibility],
    field_tys: &[&syn::Type],
    cfg: &TokenStream,
) -> (TokenStream, TokenStream) {
    if !is_tuple {
        // Named struct with zero fields: nothing to collide with, nothing to pad.
        let padded_fields = quote! { {} };
        let extra_items = quote! {
            #cfg
            const _: fn() = || {
                fn assert_impl<T: naclac_lang::prelude::Pod>() {}
                #(assert_impl::<#field_tys>();)*
            };
        };
        return (padded_fields, extra_items);
    }

    let padding_const = format_ident!("__{}_TRAILING_PADDING", ident);
    let size_sum = if field_tys.is_empty() {
        quote! { 0usize }
    } else {
        field_tys
            .iter()
            .map(|ty| quote! { ::core::mem::size_of::<#ty>() })
            .reduce(|acc, term| quote! { #acc + #term })
            .expect("field_tys is non-empty in this branch")
    };
    let align_max = align_max_expr(field_tys);

    let padded_fields = quote! {
        ( #(#field_vis #field_tys),* , pub [u8; #padding_const] )
    };

    let padding_message = format!(
        "defined_type/#[component]: `{}` has an internal padding gap — tuple structs can't have \
         padding safely inserted between positional fields (it would shift every field's index), \
         so reorder fields (largest alignment first) or use a named-field struct instead",
        ident
    );

    let extra_items = quote! {
        #cfg
        #[allow(non_upper_case_globals)]
        const #padding_const: usize = naclac_lang::prelude::compute_padding_gap(#size_sum, #align_max);

        #cfg
        const _: () = {
            assert!(
                ::core::mem::size_of::<#ident>() == (#size_sum) + #padding_const,
                #padding_message,
            );
        };

        #cfg
        const _: fn() = || {
            fn assert_impl<T: naclac_lang::prelude::Pod>() {}
            #(assert_impl::<#field_tys>();)*
        };
    };

    (padded_fields, extra_items)
}
