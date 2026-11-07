//! The `<Struct>Bumps` companion struct: the PDA bump of every seeded field,
//! plus the evaluated `realloc` size of every reallocated field, carried from
//! `load_and_validate` into `teardown`.

use {
    crate::accounts::field::{parser, realloc},
    proc_macro2::TokenStream,
    quote::quote,
};

pub(super) struct Bumps {
    pub(super) struct_name: syn::Ident,
    pub(super) struct_def: TokenStream,
    /// The expression building the struct from the locals `load_and_validate`
    /// declares (`__bump_<field>`, `__realloc_space_<field>`).
    pub(super) instantiation: TokenStream,
}

pub(super) fn build(struct_name: &syn::Ident, fields: &[parser::ParsedField]) -> Bumps {
    let mut bumps_fields = Vec::new();
    let mut bumps_instantiations = Vec::new();
    for field in fields {
        if field.pda_seed.is_some() {
            let field_name = &field.ident;
            let bump_var = quote::format_ident!("__bump_{}", field_name);
            // An optional PDA field has a bump only when its slot is present.
            if field.is_optional {
                bumps_fields.push(quote! { pub #field_name: Option<u8> });
            } else {
                bumps_fields.push(quote! { pub #field_name: u8 });
            }
            bumps_instantiations.push(quote! { #field_name: #bump_var });
        }
    }

    // `realloc = <expr>` may reference an `#[instruction(...)]` argument, which
    // exists only as a local in `load_and_validate`, while the realloc itself
    // runs in `teardown`. `Bumps` is the one value already passed between the
    // two, so the size is evaluated once here and carried in an extra field.
    for field in fields {
        if let Some(realloc_config) = &field.realloc {
            let field_name = &field.ident;
            let space_var = quote::format_ident!("__realloc_space_{}", field_name);
            let space_expr = match &realloc_config.space {
                parser::ReallocSpace::Expr(expr) => quote! { (#expr) as usize },
                parser::ReallocSpace::AnyOf(types) => realloc::generate_any_of_space_expr(
                    field_name,
                    types,
                    field.index,
                    realloc_config.grow_only,
                ),
            };
            bumps_fields.push(quote! { pub #space_var: usize });
            bumps_instantiations.push(quote! { #space_var: #space_expr });
        }
    }

    let bumps_struct_name = quote::format_ident!("{}Bumps", struct_name);
    let struct_def = quote! {
        #[derive(Clone, Copy, Default)]
        pub struct #bumps_struct_name {
            #(#bumps_fields),*
        }
    };
    let instantiation = quote! { #bumps_struct_name { #(#bumps_instantiations),* } };
    Bumps {
        struct_name: bumps_struct_name,
        struct_def,
        instantiation,
    }
}
