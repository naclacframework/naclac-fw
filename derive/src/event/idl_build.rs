//! The `idl-build` `NaclacIdlBuild` impl of an `#[event]`. With the feature
//! off it emits nothing.

use quote::quote;
#[cfg(feature = "idl-build")]
use syn::Type;

/// Builds `impl #struct_name { pub fn __naclac_idl_event(...) -> IdlEvent }`
/// — mirrors real Anchor's own `__anchor_private_gen_idl_event` exactly:
/// events get a dedicated method (not the general `NaclacIdlBuild` trait,
/// since an `IdlEvent` is never itself referenced *by* another type's
/// `create_type()`), which both returns this event's shape and recursively
/// chases `NaclacIdlBuild::create_type()` for every field that references a
/// real defined type, inserting each into the shared `types` map the caller
/// passes in. Shared by both the plain and `#[event(alloc)]` branches: the
/// field types are the same in either, and `alloc` becomes the event's
/// `alloc` flag.
#[cfg(feature = "idl-build")]
pub(super) fn idl_build_event_impl(
    struct_name: &syn::Ident,
    struct_attrs: &[syn::Attribute],
    fields: &syn::Fields,
    disc: &[u8],
    alloc: bool,
) -> proc_macro2::TokenStream {
    let struct_docs = naclac_syn::parser::extract_docs(struct_attrs);
    let constants = crate::common::crate_source::crate_constants();
    let aliases = crate::common::crate_source::crate_aliases();
    let mut field_ts_list = Vec::new();
    let mut defined_refs: Vec<Type> = Vec::new();
    for (index, f) in fields.iter().enumerate() {
        let fname = f.ident.as_ref().map_or_else(
            || index.to_string(),
            |i| naclac_syn::parser::idl_name(&i.to_string()),
        );
        let docs = naclac_syn::parser::extract_docs(&f.attrs);
        let field_ty = aliases.resolve(&f.ty);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
        let ty_json = ty_value.to_string();
        field_ts_list.push(quote! {
            naclac_lang::naclac_idl::IdlEventField {
                name: #fname.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
            }
        });
        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
            defined_refs.push(defined_ty);
        }
    }

    let struct_name_str = struct_name.to_string();

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_idl_event(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) -> naclac_lang::naclac_idl::IdlEvent {
                #(
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
                naclac_lang::naclac_idl::IdlEvent {
                    name: #struct_name_str.into(),
                    docs: vec![#(#struct_docs.into()),*],
                    discriminator: vec![#(#disc),*],
                    alloc: #alloc,
                    fields: vec![#(#field_ts_list),*],
                }
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(super) fn idl_build_event_impl(
    _struct_name: &syn::Ident,
    _struct_attrs: &[syn::Attribute],
    _fields: &syn::Fields,
    _disc: &[u8],
    _alloc: bool,
) -> proc_macro2::TokenStream {
    quote! {}
}
