//! `#[defined_type]`: a standalone fixed-size struct or enum (an instruction
//! argument, or a field nested in an account or event). The Borsh, Wincode and
//! zero-copy representations are all emitted, each behind its own
//! `#[cfg(feature = ...)]` that the calling crate's compiler resolves, since a
//! proc macro cannot see the caller's features.

mod borsh_repr;
mod idl_build;
mod validation;
mod wincode_repr;
mod zero_copy;

use proc_macro::TokenStream;

use crate::{
    common::{cfg_gates, diagnostics::reject_nonempty_attr},
    serialized_bounds, type_classify,
};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "defined_type") {
        return err;
    }
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = input.ident.clone();
    if let Some(error) = type_classify::reserved_type_name_error(&ident) {
        return error.to_compile_error().into();
    }
    if let syn::Data::Union(data) = &input.data {
        return syn::Error::new_spanned(
            data.union_token,
            "Naclac Error: `#[defined_type]` must be applied to a struct or an enum.",
        )
        .to_compile_error()
        .into();
    }
    let is_enum = matches!(&input.data, syn::Data::Enum(_));
    // The data as written, kept for the IDL and the checks: the zero-copy
    // representation later adds padding fields to `input.data`.
    let original_data = input.data.clone();
    serialized_bounds::strip_max_len(&mut input.data);

    if let Some(error) = validation::field_error(&original_data) {
        return error.to_compile_error().into();
    }

    let zero_copy_cfg = cfg_gates::zero_copy();
    let borsh_cfg = cfg_gates::borsh();
    let wincode_cfg = cfg_gates::wincode();

    let zero_copy_default_conflict =
        zero_copy::default_derive_conflict(&input, &original_data, &zero_copy_cfg);
    let borsh_variant = borsh_repr::variant(&input, is_enum, &borsh_cfg);
    let wincode_variant = wincode_repr::variant(&input, &wincode_cfg);
    let zero_copy_variant = zero_copy::variant(&mut input, &zero_copy_cfg);
    let idl_build_variant = idl_build::variant(&ident, &original_data, &input.attrs);
    let discriminant_range_conflict =
        validation::discriminant_range_conflict(&ident, &original_data, &borsh_cfg, &wincode_cfg);

    let bounds_cfg = quote::quote! {
        #[cfg(any(feature = "borsh", feature = "wincode"))]
    };
    let bounds_impl = match serialized_bounds::generate(
        &syn::DeriveInput {
            data: original_data.clone(),
            ..input.clone()
        },
        &bounds_cfg,
    ) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    };

    quote::quote! {
        #discriminant_range_conflict
        #zero_copy_default_conflict
        #bounds_impl
        #borsh_variant
        #wincode_variant
        #zero_copy_variant
        #idl_build_variant
    }
    .into()
}
