//! The `idl-build` assembler of `#[program]`: collects what each instruction
//! declares, and what every `#[account]`, `#[event]`, `#[error_code]` and
//! `#[constant]` registered, into one `pub fn __naclac_print_idl()` (called by
//! the program's `idl-build` `[[bin]]`). Without the `idl-build` feature only
//! an empty stub exists.

use quote::quote;

/// One instruction's worth of data the main `#[program]` loop already extracts
/// for dispatch codegen, captured a second time (owned, cloned) for the
/// idl-build assembler built after that loop finishes.
#[cfg(feature = "idl-build")]
pub(super) struct IdlBuildIx {
    pub(super) name: syn::Ident,
    pub(super) docs: Vec<String>,
    pub(super) disc: Vec<u8>,
    pub(super) ctx_struct: syn::Path,
    pub(super) arg_names: Vec<syn::Ident>,
    pub(super) arg_types: Vec<syn::Type>,
    pub(super) return_ty: Option<syn::Type>,
}

/// Builds the whole `idl-build` assembler: a plain `pub fn __naclac_print_idl()`
/// at crate-root scope, called directly by the program's `[[bin]]` entry point.
///
/// The accounts, events, errors and constants come from
/// `naclac_idl::registry::collect()`, which every item's own macro expansion
/// feeds. Each instruction contributes its accounts
/// (`__naclac_idl_accounts()`, with seed field types patched in from the
/// collected account types; see `idl/src/idl_build_accounts.rs`), its args and
/// return type, whose defined types are chased through `NaclacIdlType`, and
/// the named constants its PDA seeds use. The address comes from `crate::ID`.
///
/// It exists only under `#[cfg(feature = "idl-build")]`, with an empty stub
/// otherwise; a runtime check of `CARGO_FEATURE_IDL_BUILD` would not work, as
/// a proc macro's process does not receive `CARGO_FEATURE_*` variables.
#[cfg(feature = "idl-build")]
pub(super) fn build_idl_assembler(ixs: &[IdlBuildIx]) -> proc_macro2::TokenStream {
    let idl = quote! { naclac_lang::naclac_idl };

    let mut ix_tokens = Vec::new();

    for ix in ixs {
        let ix_name_str = ix.name.to_string();
        let ix_docs = &ix.docs;
        let ctx_struct = &ix.ctx_struct;
        let disc = &ix.disc;

        let mut arg_field_tokens = Vec::new();
        let mut chase_calls = Vec::new();
        for (arg_name, arg_ty) in ix.arg_names.iter().zip(ix.arg_types.iter()) {
            let arg_name_str = naclac_syn::parser::idl_name(&arg_name.to_string());
            arg_field_tokens.push(quote! {
                #idl::IdlField {
                    name: #arg_name_str.into(),
                    docs: vec![],
                    ty: <#arg_ty as #idl::idl_type::NaclacIdlType>::idl_type(),
                    slots: <#arg_ty as #idl::idl_type::NaclacIdlType>::slots(),
                    max_len: <#arg_ty as #idl::idl_type::NaclacIdlType>::max_len(),
                }
            });
            chase_calls.push(quote! {
                <#arg_ty as #idl::idl_type::NaclacIdlType>::chase(&mut __defined_types);
            });
        }

        let returns_tokens = match &ix.return_ty {
            Some(return_ty) => {
                chase_calls.push(quote! {
                    <#return_ty as #idl::idl_type::NaclacIdlType>::chase(&mut __defined_types);
                });
                quote! { Some(<#return_ty as #idl::idl_type::NaclacIdlType>::idl_type()) }
            }
            None => quote! { None },
        };

        ix_tokens.push(quote! {
            {
                #(#chase_calls)*
                let __built_accounts = #ctx_struct::__naclac_idl_accounts();
                for __built in &__built_accounts {
                    __seed_constants.extend(__built.seed_constants());
                }
                #idl::IdlInstruction {
                    name: #ix_name_str.into(),
                    docs: vec![#(#ix_docs.into()),*],
                    discriminator: vec![#(#disc),*],
                    accounts: __built_accounts
                        .into_iter()
                        .map(|a| a.resolve(&__account_types))
                        .collect(),
                    args: vec![#(#arg_field_tokens),*],
                    returns: #returns_tokens,
                }
            }
        });
    }

    quote! {
        #[cfg(feature = "idl-build")]
        pub fn __naclac_print_idl() {
            let #idl::registry::Collected {
                account_types: __account_types,
                account_discriminators: __account_discs,
                defined_types: mut __defined_types,
                events: __events,
                errors: __errors,
                constants: mut __constants,
            } = #idl::registry::collect();
            let mut __seed_constants: naclac_lang::prelude::Vec<#idl::IdlConstant> = vec![];

            let __instructions: naclac_lang::prelude::Vec<#idl::IdlInstruction> =
                vec![#(#ix_tokens),*];

            let __accounts: naclac_lang::prelude::Vec<#idl::IdlAccountStruct> = __account_types
                .iter()
                .filter_map(|(name, ty)| {
                    let #idl::IdlTypeDef::Struct { fields, docs } = ty else {
                        return None;
                    };
                    let Some(discriminator) = __account_discs.get(name) else {
                        panic!("Naclac Error: the IDL account `{}` has no discriminator.", name);
                    };
                    Some(#idl::IdlAccountStruct {
                        name: name.clone(),
                        docs: docs.clone(),
                        discriminator: discriminator.clone(),
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

            for __seed_constant in __seed_constants {
                if !__constants.iter().any(|c| c.name == __seed_constant.name) {
                    __constants.push(__seed_constant);
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
