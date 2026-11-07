//! The `idl-build` `NaclacIdlBuild` impl of an `#[account]` struct (and of an
//! `#[instruction_args]` struct, which reuses it). With the feature off it
//! emits nothing.

use quote::quote;

/// Builds `impl NaclacIdlBuild for #struct_name`, gated
/// `#[cfg(feature = "idl-build")]` in the OUTPUT tokens. Built once, from
/// `fields` (the struct's own already-parsed `syn::Fields`, before padding/
/// `#[max_len]` stripping) — never re-parses anything from disk, and is
/// identical regardless of which representation (Pod/Borsh) the calling
/// crate actually compiles, since the IDL-visible shape doesn't depend on
/// representation. Field type classification reuses
/// `naclac_syn::parser::rust_type_to_idl` — a pure matcher on the
/// already-in-hand syntax for a field's *own* shape — computed here, now, at this struct's macro-expansion time,
/// since a field's own top-level type is fully known from its syntax alone.
/// Only a *referenced* type's internal shape is deferred to real trait
/// dispatch at `idl-build` test-runtime (via `insert_types`, chasing
/// `<Referenced>::create_type()`), since that's the part a from-syntax-alone
/// mapper cannot know without either re-parsing elsewhere or waiting for the
/// referenced type's own compiled impl to exist.
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
    let constants = crate::common::crate_source::crate_constants();
    let aliases = crate::common::crate_source::crate_aliases();
    let mut idl_fields = Vec::new();
    let mut defined_refs: Vec<syn::Type> = Vec::new();

    for (index, field) in fields.iter().enumerate() {
        let name = field.ident.as_ref().map_or_else(
            || index.to_string(),
            |ident| naclac_syn::parser::idl_name(&ident.to_string()),
        );
        let docs = naclac_syn::parser::extract_docs(&field.attrs);
        let field_ty = aliases.resolve(&field.ty);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
        let ty_json = ty_value.to_string();
        let slots = match naclac_syn::parser::rust_type_slots(&field_ty, &constants) {
            Some(slots) => quote! { Some(#slots) },
            None => quote! { None },
        };

        idl_fields.push(quote! {
            naclac_lang::naclac_idl::IdlField {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
                slots: #slots,
            }
        });

        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
            defined_refs.push(defined_ty);
        }
    }

    let struct_name_str = struct_name.to_string();

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
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
            }

            fn get_full_path() -> naclac_lang::naclac_idl::__private::String {
                #struct_name_str.into()
            }

            #account_disc_fn
        }
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
