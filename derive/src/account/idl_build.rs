//! The `idl-build` IDL impls of an `#[account]` struct (and of an
//! `#[instruction_args]` struct, which reuses it): `NaclacIdlBuild`,
//! `NaclacIdlType`, and for an account a registered `IdlFragment`. With the
//! feature off it emits nothing.

use quote::quote;

/// Builds `impl NaclacIdlBuild` and `impl NaclacIdlType for #struct_name`, plus
/// an `IdlFragment::Account` registration when `account_disc` is given, all
/// gated `#[cfg(feature = "idl-build")]` in the OUTPUT tokens.
///
/// Every field type is described through `NaclacIdlType`, so the compiler
/// resolves aliases and constants; the macro never inspects a type's name.
/// The IDL shape is the same whichever representation (Pod/Borsh) the crate
/// compiles.
#[cfg(feature = "idl-build")]
pub(crate) fn idl_build_impl(
    struct_name: &syn::Ident,
    generics: &syn::Generics,
    struct_attrs: &[syn::Attribute],
    fields: &syn::Fields,
    account_disc: Option<&[u8]>,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let struct_docs = naclac_syn::parser::extract_docs(struct_attrs);
    let account_disc_fn = match account_disc {
        Some(disc) => quote! {
            fn account_discriminator() -> Option<naclac_lang::prelude::Vec<u8>> {
                Some(vec![#(#disc),*])
            }
        },
        None => quote! {},
    };
    let mut idl_fields = Vec::new();
    let mut field_types: Vec<&syn::Type> = Vec::new();

    for (index, field) in fields.iter().enumerate() {
        let name = field.ident.as_ref().map_or_else(
            || index.to_string(),
            |ident| naclac_syn::parser::idl_name(&ident.to_string()),
        );
        let docs = naclac_syn::parser::extract_docs(&field.attrs);
        let ty = &field.ty;
        idl_fields.push(quote! {
            naclac_lang::naclac_idl::IdlField {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: <#ty as naclac_lang::naclac_idl::idl_type::NaclacIdlType>::idl_type(),
                slots: <#ty as naclac_lang::naclac_idl::idl_type::NaclacIdlType>::slots(),
            }
        });
        field_types.push(ty);
    }

    let struct_name_str = struct_name.to_string();

    let registration = match account_disc {
        Some(_) => quote! {
            #[cfg(feature = "idl-build")]
            const _: () = {
                fn build(
                    account_types: &mut naclac_lang::naclac_idl::idl_type::Types,
                    discriminators: &mut naclac_lang::naclac_idl::registry::Discriminators,
                    defined_types: &mut naclac_lang::naclac_idl::idl_type::Types,
                ) {
                    use naclac_lang::naclac_idl::idl_build::NaclacIdlBuild;
                    let name = <#struct_name as NaclacIdlBuild>::get_full_path();
                    if let Some(definition) = <#struct_name as NaclacIdlBuild>::create_type() {
                        account_types.insert(name.clone(), definition);
                    }
                    if let Some(disc) = <#struct_name as NaclacIdlBuild>::account_discriminator() {
                        discriminators.insert(name, disc);
                    }
                    <#struct_name as NaclacIdlBuild>::insert_types(defined_types);
                }
                naclac_lang::naclac_idl::__private::inventory::submit! {
                    naclac_lang::naclac_idl::registry::IdlFragment::Account {
                        name: #struct_name_str,
                        build,
                    }
                }
            };
        },
        None => quote! {},
    };

    quote! {
        #[cfg(feature = "idl-build")]
        impl #impl_generics naclac_lang::naclac_idl::idl_build::NaclacIdlBuild for #struct_name #ty_generics #where_clause {
            fn create_type() -> Option<naclac_lang::naclac_idl::IdlTypeDef> {
                Some(naclac_lang::naclac_idl::IdlTypeDef::Struct {
                    fields: vec![#(#idl_fields),*],
                    docs: vec![#(#struct_docs.into()),*],
                })
            }

            fn insert_types(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) {
                #(
                    <#field_types as naclac_lang::naclac_idl::idl_type::NaclacIdlType>::chase(types);
                )*
            }

            fn get_full_path() -> naclac_lang::naclac_idl::__private::String {
                #struct_name_str.into()
            }

            #account_disc_fn
        }

        #[cfg(feature = "idl-build")]
        impl #impl_generics naclac_lang::naclac_idl::idl_type::NaclacIdlType for #struct_name #ty_generics #where_clause {
            fn idl_type() -> naclac_lang::naclac_idl::idl_type::IdlJson {
                naclac_lang::naclac_idl::idl_type::defined(#struct_name_str)
            }

            fn chase(types: &mut naclac_lang::naclac_idl::idl_type::Types) {
                naclac_lang::naclac_idl::idl_type::chase_defined::<Self>(types);
            }
        }

        #registration
    }
}

#[cfg(not(feature = "idl-build"))]
pub(crate) fn idl_build_impl(
    _struct_name: &syn::Ident,
    _generics: &syn::Generics,
    _struct_attrs: &[syn::Attribute],
    _fields: &syn::Fields,
    _account_disc: Option<&[u8]>,
) -> proc_macro2::TokenStream {
    quote! {}
}
