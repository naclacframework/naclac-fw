//! The `idl-build` assembler of `#[program]`: collects what each instruction
//! declares into one `pub fn __naclac_print_idl()` (called by the program's
//! `idl-build` `[[bin]]`), plus the compile errors for IDL items the crate
//! cannot name. Without the `idl-build` feature only an empty stub exists.

use quote::quote;

/// Real `idl-build` assembler. One instruction's worth of
/// data the main `#[program]` loop already extracts for dispatch codegen,
/// captured a second time (owned, cloned) for the idl-build assembler built
/// after that loop finishes.
#[cfg(feature = "idl-build")]
pub(super) struct IdlBuildIx {
    pub(super) name: syn::Ident,
    pub(super) disc: Vec<u8>,
    pub(super) ctx_struct: syn::Path,
    pub(super) arg_names: Vec<syn::Ident>,
    pub(super) arg_types: Vec<syn::Type>,
    pub(super) return_ty: Option<syn::Type>,
}

/// A compile error, gated on the item's own `cfg`s, for an IDL item that
/// generated code cannot name because a module on its path (or the item
/// itself) is not visible at the crate root.
#[cfg(feature = "idl-build")]
fn unreachable_item_error(
    kind: &str,
    item: &naclac_syn::names::TaggedItem,
) -> proc_macro2::TokenStream {
    let message = format!(
        "Naclac Error: the IDL cannot reach {kind} `{}::{}`. Declare it, and every module on \
         its path, `pub` or `pub(crate)` so the `idl-build` assembler can name it.",
        item.module_path, item.name
    );
    let cfgs = &item.cfgs;
    quote! { #(#cfgs)* ::core::compile_error!(#message); }
}

/// One compile error per rule of `naclac_syn::idl_items` the crate breaks,
/// each gated on `all(..)` of the cfgs under which the clashing declarations
/// coexist, so the compiler decides whether the build is affected. A
/// malformed `#[error_code]` argument is left to that macro to report.
pub(super) fn idl_rule_errors() -> proc_macro2::TokenStream {
    let modules = naclac_syn::modules::crate_modules(&crate::common::crate_source::manifest_dir());
    let Ok(declarations) = naclac_syn::idl_items::declarations(&modules) else {
        return quote! {};
    };
    let errors = naclac_syn::idl_items::violations(&declarations)
        .into_iter()
        .map(|violation| {
            let message = violation.message;
            let predicates: Vec<proc_macro2::TokenStream> = violation
                .cfgs
                .iter()
                .filter_map(|attr| match &attr.meta {
                    syn::Meta::List(list) => Some(list.tokens.clone()),
                    _ => None,
                })
                .collect();
            if predicates.is_empty() {
                quote! { ::core::compile_error!(#message); }
            } else {
                quote! { #[cfg(all(#(#predicates),*))] ::core::compile_error!(#message); }
            }
        });
    quote! { #(#errors)* }
}

/// Builds the whole `idl-build` assembler: a plain `pub fn __naclac_print_idl()`
/// at crate-root scope, called directly by the program's `[[bin]]` entry point.
/// It is never `#[test]`-tagged, as a `[[bin]]` `main()` cannot call a `#[test]`
/// function.
///
/// It chases every reachable account type through each instruction's
/// `__naclac_insert_account_types`, resolves each instruction's accounts
/// (`__naclac_idl_accounts()`, patching in seed field types and constant values
/// from the assembled type map; see `idl/src/idl_build_accounts.rs`), resolves
/// instruction args and chases any `#[defined_type]`, and reads `crate::ID`
/// for the address. Constants, events and errors are never named by an
/// instruction signature, so a name-and-module-path scan
/// (`naclac_syn::names`) finds them and calls their print functions by path.
///
/// It exists only under `#[cfg(feature = "idl-build")]`, with an empty stub
/// otherwise; a runtime check of `CARGO_FEATURE_IDL_BUILD` would not work, as
/// a proc macro's process does not receive `CARGO_FEATURE_*` variables.
#[cfg(feature = "idl-build")]
pub(super) fn build_idl_assembler(ixs: &[IdlBuildIx]) -> proc_macro2::TokenStream {
    let idl = quote! { naclac_lang::naclac_idl };

    let handler_names: Vec<String> = ixs.iter().map(|ix| ix.name.to_string()).collect();
    let mut instruction_docs =
        naclac_syn::instruction_docs(&crate::common::crate_source::manifest_dir(), &handler_names);

    let constants = crate::common::crate_source::crate_constants();
    let aliases = crate::common::crate_source::crate_aliases();
    let mut insert_account_types_calls = Vec::new();
    let mut insert_defined_type_calls = Vec::new();
    let mut ix_tokens = Vec::new();

    for ix in ixs {
        let ix_name_str = ix.name.to_string();
        let ix_docs = instruction_docs.remove(&ix_name_str).unwrap_or_default();
        let ctx_struct = &ix.ctx_struct;
        let disc = &ix.disc;

        insert_account_types_calls.push(quote! {
            #ctx_struct::__naclac_insert_account_types(
                &mut __account_types,
                &mut __account_discs,
                &mut __defined_types,
            );
        });

        let mut arg_field_tokens = Vec::new();
        for (arg_name, arg_ty) in ix.arg_names.iter().zip(ix.arg_types.iter()) {
            let arg_ty = &aliases.resolve(arg_ty);
            let arg_name_str = naclac_syn::parser::idl_name(&arg_name.to_string());
            let ty_value = naclac_syn::parser::rust_type_to_idl(arg_ty, &constants);
            let ty_json = ty_value.to_string();
            let slots = match naclac_syn::parser::rust_type_slots(arg_ty, &constants) {
                Some(slots) => quote! { Some(#slots) },
                None => quote! { None },
            };
            arg_field_tokens.push(quote! {
                #idl::IdlField {
                    name: #arg_name_str.into(),
                    docs: vec![],
                    ty: #idl::serde_json::from_str(#ty_json)
                        .expect("naclac idl-build: generated arg type JSON must parse"),
                    slots: #slots,
                }
            });

            if let Some(leaf_ty) = naclac_syn::parser::defined_type_leaf(arg_ty) {
                insert_defined_type_calls.push(quote! {
                    if let Some(ty) = <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::create_type() {
                        __defined_types.insert(<#leaf_ty as #idl::idl_build::NaclacIdlBuild>::get_full_path(), ty);
                        <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::insert_types(&mut __defined_types);
                    }
                });
            }
        }

        let returns_tokens = match &ix.return_ty {
            Some(return_ty) => {
                let return_ty = &aliases.resolve(return_ty);
                let return_json =
                    naclac_syn::parser::rust_type_to_idl(return_ty, &constants).to_string();
                if let Some(leaf_ty) = naclac_syn::parser::defined_type_leaf(return_ty) {
                    insert_defined_type_calls.push(quote! {
                        if let Some(ty) = <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::create_type() {
                            __defined_types.insert(<#leaf_ty as #idl::idl_build::NaclacIdlBuild>::get_full_path(), ty);
                            <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::insert_types(&mut __defined_types);
                        }
                    });
                }
                quote! {
                    Some(#idl::serde_json::from_str(#return_json)
                        .expect("naclac idl-build: generated return type JSON must parse"))
                }
            }
            None => quote! { None },
        };

        ix_tokens.push(quote! {
            #idl::IdlInstruction {
                name: #ix_name_str.into(),
                docs: vec![#(#ix_docs.into()),*],
                discriminator: vec![#(#disc),*],
                accounts: #ctx_struct::__naclac_idl_accounts()
                    .into_iter()
                    .map(|a| a.resolve(&__account_types))
                    .collect(),
                args: vec![#(#arg_field_tokens),*],
                returns: #returns_tokens,
            }
        });
    }

    let mut account_calls = Vec::new();
    let mut event_calls = Vec::new();
    let mut error_calls = Vec::new();
    let mut const_calls = Vec::new();
    let mut crate_const_pushes = Vec::new();
    {
        let manifest_dir = crate::common::crate_source::manifest_dir();
        let program_dir = manifest_dir.as_path();

        for item in naclac_syn::names::scan_tagged_items(program_dir, "account") {
            let Ok(path) =
                syn::parse_str::<syn::Path>(&format!("{}::{}", item.module_path, item.name))
            else {
                continue;
            };
            let cfgs = &item.cfgs;
            account_calls.push(if item.visible_from_root {
                quote! {
                    #(#cfgs)*
                    __naclac_insert_account::<#path>(
                        &mut __account_types,
                        &mut __account_discs,
                        &mut __defined_types,
                    );
                }
            } else {
                unreachable_item_error("account", &item)
            });
        }

        for item in naclac_syn::names::scan_tagged_items(program_dir, "event") {
            let Ok(path) =
                syn::parse_str::<syn::Path>(&format!("{}::{}", item.module_path, item.name))
            else {
                continue;
            };
            let cfgs = &item.cfgs;
            event_calls.push(if item.visible_from_root {
                quote! { #(#cfgs)* __events.push(#path::__naclac_idl_event(&mut __defined_types)); }
            } else {
                unreachable_item_error("event", &item)
            });
        }

        for item in naclac_syn::names::scan_tagged_items(program_dir, "error_code") {
            let Ok(path) =
                syn::parse_str::<syn::Path>(&format!("{}::{}", item.module_path, item.name))
            else {
                continue;
            };
            let cfgs = &item.cfgs;
            error_calls.push(if item.visible_from_root {
                quote! { #(#cfgs)* __errors.extend(#path::__naclac_idl_errors()); }
            } else {
                unreachable_item_error("error enum", &item)
            });
        }

        for mut item in naclac_syn::names::scan_untagged_pub_consts(program_dir) {
            item.ty = aliases.resolve(&item.ty);
            if naclac_syn::parser::defined_type_leaf(&item.ty).is_some() || !item.visible_from_root
            {
                continue;
            }
            let cfgs = item.cfgs;
            let Ok(path) =
                syn::parse_str::<syn::Path>(&format!("{}::{}", item.module_path, item.name))
            else {
                continue;
            };
            let ty_value = naclac_syn::parser::rust_type_to_idl(&item.ty, &constants);
            let ty_json = ty_value.to_string();
            let name = item.name;
            let docs = item.docs;
            let value = crate::constant::constant_value_tokens(&item.ty, &quote! { #path });
            crate_const_pushes.push(quote! {
                #(#cfgs)*
                __crate_consts.push(#idl::IdlConstant {
                    name: #name.into(),
                    docs: vec![#(#docs.into()),*],
                    ty: #idl::serde_json::from_str(#ty_json)
                        .expect("naclac idl-build: generated constant type JSON must parse"),
                    value: #value,
                });
            });
        }

        for item in naclac_syn::names::scan_tagged_items(program_dir, "constant") {
            // Lowercased to match the name `constant.rs` generates.
            let fn_path = match syn::parse_str::<syn::Path>(&format!(
                "{}::__naclac_idl_print_const_{}",
                item.module_path,
                item.name.to_lowercase()
            )) {
                Ok(path) => path,
                Err(error) => return error.to_compile_error(),
            };
            let cfgs = &item.cfgs;
            const_calls.push(if item.visible_from_root {
                quote! { #(#cfgs)* __constants.push(#fn_path()); }
            } else {
                unreachable_item_error("constant", &item)
            });
        }
    }

    quote! {
        #[cfg(feature = "idl-build")]
        pub fn __naclac_print_idl() {
            let mut __account_types: #idl::__private::BTreeMap<#idl::__private::String, #idl::IdlTypeDef> =
                #idl::__private::BTreeMap::new();
            let mut __account_discs: #idl::__private::BTreeMap<#idl::__private::String, naclac_lang::prelude::Vec<u8>> =
                #idl::__private::BTreeMap::new();
            let mut __defined_types: #idl::__private::BTreeMap<#idl::__private::String, #idl::IdlTypeDef> =
                #idl::__private::BTreeMap::new();
            let mut __events: naclac_lang::prelude::Vec<#idl::IdlEvent> = vec![];
            let mut __errors: naclac_lang::prelude::Vec<#idl::IdlError> = vec![];
            let mut __constants: naclac_lang::prelude::Vec<#idl::IdlConstant> = vec![];
            let mut __crate_consts: naclac_lang::prelude::Vec<#idl::IdlConstant> = vec![];
            #(#crate_const_pushes)*

            fn __naclac_insert_account<T: #idl::idl_build::NaclacIdlBuild>(
                types: &mut #idl::__private::BTreeMap<#idl::__private::String, #idl::IdlTypeDef>,
                discriminators: &mut #idl::__private::BTreeMap<#idl::__private::String, naclac_lang::prelude::Vec<u8>>,
                defined_types: &mut #idl::__private::BTreeMap<#idl::__private::String, #idl::IdlTypeDef>,
            ) {
                if let Some(ty) = T::create_type() {
                    types.insert(T::get_full_path(), ty);
                    if let Some(disc) = T::account_discriminator() {
                        discriminators.insert(T::get_full_path(), disc);
                    }
                    T::insert_types(defined_types);
                }
            }

            // Every map/list-populating call runs first — instructions,
            // accounts, and defined_types are all assembled from these
            // maps afterward, so nothing here can run after that read.
            #(#account_calls)*
            #(#insert_account_types_calls)*
            #(#insert_defined_type_calls)*
            #(#event_calls)*
            #(#error_calls)*
            #(#const_calls)*

            let __instructions: naclac_lang::prelude::Vec<#idl::IdlInstruction> =
                vec![#(#ix_tokens),*];

            let __accounts: naclac_lang::prelude::Vec<#idl::IdlAccountStruct> = __account_types
                .iter()
                .filter_map(|(name, ty)| {
                    let #idl::IdlTypeDef::Struct { fields, docs } = ty else {
                        return None;
                    };
                    Some(#idl::IdlAccountStruct {
                        name: name.clone(),
                        docs: docs.clone(),
                        discriminator: __account_discs.get(name).cloned().unwrap_or_else(|| {
                            naclac_lang::naclac_syn::discriminator::compute_discriminator("account", name).to_vec()
                        }),
                        ty: #idl::IdlTypeStruct {
                            kind: "struct".into(),
                            fields: fields.clone(),
                        },
                    })
                })
                .collect();

            let __defined_types_list: naclac_lang::prelude::Vec<#idl::IdlType> = __defined_types
                .iter()
                .map(|(name, ty)| #idl::IdlType {
                    name: name.clone(),
                    docs: match ty {
                        #idl::IdlTypeDef::Struct { docs, .. } | #idl::IdlTypeDef::Enum { docs, .. } => docs.clone(),
                    },
                    ty: ty.clone(),
                })
                .collect();

            // Top-level `pdas`:
            // deduplicate every instruction's per-account PDA by account
            // name, from data already resolved above — no new discovery
            // needed, unlike constants/events/errors.
            let mut __pda_map: #idl::__private::BTreeMap<
                #idl::__private::String,
                (naclac_lang::prelude::Vec<#idl::IdlSeed>, Option<#idl::IdlSeed>),
            > = #idl::__private::BTreeMap::new();
            for __ix in &__instructions {
                for __acc in &__ix.accounts {
                    if let Some(__pda) = &__acc.pda {
                        __pda_map
                            .entry(__acc.name.clone())
                            .or_insert_with(|| (__pda.seeds.clone(), __pda.program.clone()));
                    }
                }
            }
            let __pdas: naclac_lang::prelude::Vec<#idl::IdlPdaDef> = __pda_map
                .into_iter()
                .map(|(name, (seeds, program))| #idl::IdlPdaDef { name, seeds, program })
                .collect();

            // A seed constant not `#[constant]`-tagged still needs to end
            // up in the top-level `constants` list. Its declared type
            // and value come from evaluating the crate's own `pub const`
            // (`__crate_consts`), deduplicated against any already-tagged
            // entry by name; a name with no such constant falls back to the
            // seed's bytes.
            for __ix in &__instructions {
                for __acc in &__ix.accounts {
                    let Some(__pda) = &__acc.pda else { continue };
                    for __seed in __pda.seeds.iter().chain(__pda.program.iter()) {
                        let #idl::IdlSeed::Const { value, name: Some(__n) } = __seed else {
                            continue;
                        };
                        if __constants.iter().any(|c| &c.name == __n) {
                            continue;
                        }
                        match __crate_consts.iter().find(|c| &c.name == __n) {
                            Some(__crate_const) => __constants.push(__crate_const.clone()),
                            None => __constants.push(#idl::IdlConstant {
                                name: __n.clone(),
                                docs: vec![],
                                ty: #idl::serde_json::Value::String("bytes".into()),
                                value: format!("{:?}", value),
                            }),
                        }
                    }
                }
            }

            #[cfg(feature = "bytemuck")]
            let __serialization = #idl::Serialization::Bytemuck;
            #[cfg(feature = "borsh")]
            let __serialization = #idl::Serialization::Borsh;
            #[cfg(feature = "wincode")]
            let __serialization = #idl::Serialization::Wincode;

            let mut __idl = #idl::Idl {
                address: naclac_lang::prelude::base58::encode_address_to_string(
                    &naclac_lang::prelude::ToAddress::address(&crate::ID),
                ),
                metadata: #idl::IdlMetadata {
                    name: env!("CARGO_PKG_NAME").into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    description: concat!(env!("CARGO_PKG_NAME"), " — Generated by Naclac Framework").into(),
                    serialization: __serialization,
                },
                instructions: __instructions,
                accounts: __accounts,
                events: __events,
                errors: __errors,
                constants: __constants,
                defined_types: __defined_types_list,
                pdas: __pdas,
                event_cpi_tag: if crate::__NACLAC_EVENT_CPI_IN_USE {
                    Some(crate::__NACLAC_EVENT_DISPATCH_TAG.to_vec())
                } else {
                    None
                },
            };

            __idl.canonicalize();

            extern crate std;
            std::println!("{}", #idl::serde_json::to_string_pretty(&__idl).unwrap());
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(super) struct IdlBuildIx;

#[cfg(not(feature = "idl-build"))]
pub(super) fn build_idl_assembler(_ixs: &[IdlBuildIx]) -> proc_macro2::TokenStream {
    quote! {}
}
