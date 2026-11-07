//! The `idl-build` `NaclacIdlBuild` impl for a `#[defined_type]`. Without the
//! `idl-build` feature every function here emits nothing.

use proc_macro2::TokenStream;

/// The `NaclacIdlBuild` impl for the type, per its original data.
///
/// `attrs` must be the item's final attribute list, after the zero-copy
/// representation added any default `#[repr(u8)]`, since the IDL's recorded
/// enum repr must match the compiled discriminant width.
pub(super) fn variant(
    ident: &syn::Ident,
    original_data: &syn::Data,
    attrs: &[syn::Attribute],
) -> TokenStream {
    match original_data {
        syn::Data::Struct(s) => {
            crate::account::idl_build_impl(ident, &syn::Generics::default(), attrs, &s.fields, None)
        }
        syn::Data::Enum(e) => enum_impl(ident, &e.variants, attrs),
        syn::Data::Union(_) => quote::quote! {},
    }
}

/// `impl NaclacIdlBuild` for a `#[defined_type]` enum, producing
/// `IdlTypeDef::Enum { variants, repr }`.
#[cfg(feature = "idl-build")]
fn enum_impl(
    ident: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    attrs: &[syn::Attribute],
) -> TokenStream {
    let enum_docs = naclac_syn::parser::extract_docs(attrs);
    let repr =
        naclac_syn::discriminator::detect_enum_repr(attrs).unwrap_or_else(|| "u8".to_string());
    let repr_tokens = quote::quote! { Some(#repr.to_string()) };

    let discriminants = naclac_syn::discriminator::variant_discriminants(variants.iter());
    let constants = crate::common::crate_source::crate_constants();
    let aliases = crate::common::crate_source::crate_aliases();
    let mut idl_variants = Vec::new();
    let mut defined_refs: Vec<syn::Type> = Vec::new();

    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_name = variant.ident.to_string();
        let discriminant_str = discriminant.to_string();
        let variant_docs = naclac_syn::parser::extract_docs(&variant.attrs);

        let fields_ts = match &variant.fields {
            syn::Fields::Unit => quote::quote! { None },
            syn::Fields::Named(named) => {
                let mut field_ts_list = Vec::new();
                for f in &named.named {
                    let fname =
                        naclac_syn::parser::idl_name(&f.ident.as_ref().unwrap().to_string());
                    let field_docs = naclac_syn::parser::extract_docs(&f.attrs);
                    let field_ty = aliases.resolve(&f.ty);
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
                    let ty_json = ty_value.to_string();
                    field_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::IdlField {
                            name: #fname.into(),
                            docs: vec![#(#field_docs.into()),*],
                            ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                                .expect("naclac idl-build: generated field type JSON must parse"),
                            slots: None,
                        }
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
                        defined_refs.push(defined_ty);
                    }
                }
                quote::quote! {
                    Some(naclac_lang::naclac_idl::IdlEnumFields::Named(vec![#(#field_ts_list),*]))
                }
            }
            syn::Fields::Unnamed(unnamed) => {
                let mut ty_ts_list = Vec::new();
                for f in &unnamed.unnamed {
                    let field_ty = aliases.resolve(&f.ty);
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
                    let ty_json = ty_value.to_string();
                    ty_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                            .expect("naclac idl-build: generated field type JSON must parse")
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
                        defined_refs.push(defined_ty);
                    }
                }
                quote::quote! {
                    Some(naclac_lang::naclac_idl::IdlEnumFields::Tuple(vec![#(#ty_ts_list),*]))
                }
            }
        };

        idl_variants.push(quote::quote! {
            naclac_lang::naclac_idl::IdlEnumVariant {
                name: #variant_name.into(),
                docs: vec![#(#variant_docs.into()),*],
                fields: #fields_ts,
                discriminant: #discriminant_str.into(),
            }
        });
    }

    let ident_str = ident.to_string();

    quote::quote! {
        #[cfg(feature = "idl-build")]
        impl naclac_lang::naclac_idl::idl_build::NaclacIdlBuild for #ident {
            fn create_type() -> Option<naclac_lang::naclac_idl::IdlTypeDef> {
                Some(naclac_lang::naclac_idl::IdlTypeDef::Enum {
                    variants: vec![#(#idl_variants),*],
                    docs: vec![#(#enum_docs.into()),*],
                    repr: #repr_tokens,
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
                #ident_str.into()
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn enum_impl(
    _ident: &syn::Ident,
    _variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    _attrs: &[syn::Attribute],
) -> TokenStream {
    quote::quote! {}
}
