//! `impl <Accounts struct> { fn __naclac_insert_account_types() }`: lets
//! `#[program]`'s assembler discover which `#[account]` types (each needing a
//! discriminator-bearing `Idl.accounts` entry) an instruction references.

use {crate::accounts::field::parser, quote::quote};

/// If `ty` is (optionally boxed) `Account<T>`/`InterfaceAccount<T>`, returns
/// `T`, the `#[account]` type the field holds. `Box<Account<T>>` unwraps like
/// bare `Account<T>`; every other shape (`Signer`, `Program<T>`, `Interface<T>`,
/// plain `AccountInfo`/`AccountView`) returns `None`, as those have no
/// `#[account]` data of their own.
#[cfg(feature = "idl-build")]
fn account_wrapper_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
    let mut ty = ty;
    while let syn::Type::Path(type_path) = ty {
        if type_path.path.segments.last()?.ident != "Box" {
            break;
        }
        ty = crate::type_classify::first_generic_type(ty)?;
    }
    let base = crate::type_classify::base_ident(ty)?.to_string();
    if base == "Account" || base == "InterfaceAccount" {
        return crate::type_classify::first_generic_type(ty);
    }
    None
}

/// Builds `impl #struct_name { pub fn __naclac_insert_account_types(types, discriminators, defined_types) }`.
///
/// It chases every type reachable from this struct's fields through
/// `NaclacIdlBuild`, the same way `account.rs`'s `insert_types` does for an
/// `#[account]` struct. The `#[account]` types land in `types`; the defined
/// types they reference land in `defined_types`, because only an `#[account]`
/// type has a discriminator. Emits nothing without the `idl-build` feature.
#[cfg(feature = "idl-build")]
pub(crate) fn idl_insert_account_types_impl(
    struct_name: &syn::Ident,
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    let aliases = crate::common::crate_source::crate_aliases();
    let field_tys: Vec<syn::Type> = parsed_fields
        .iter()
        .map(|f| aliases.resolve(&f.ty))
        .collect();
    let account_tys: Vec<&syn::Type> = field_tys
        .iter()
        .filter_map(account_wrapper_inner_type)
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_insert_account_types(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
                discriminators: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::prelude::Vec<u8>,
                >,
                defined_types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) {
                #(
                    if let Some(ty) = <#account_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#account_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        if let Some(disc) = <#account_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::account_discriminator() {
                            discriminators.insert(
                                <#account_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                                disc,
                            );
                        }
                        <#account_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(defined_types);
                    }
                )*
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(crate) fn idl_insert_account_types_impl(
    _struct_name: &syn::Ident,
    _parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    quote! {}
}
