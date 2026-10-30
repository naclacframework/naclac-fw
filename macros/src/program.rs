//! # Program Macro Logic
//!
//! Handles the expansion of the `#[program]` macro, serving as the core router
//! and instruction dispatcher for the smart contract across all execution modes.

use {
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, FnArg, Item, ItemMod, Pat, Type},
};

/// Real `idl-build` assembler. One instruction's worth of
/// data the main `#[program]` loop already extracts for dispatch codegen,
/// captured a second time (owned, cloned) for the idl-build assembler built
/// after that loop finishes.
#[cfg(feature = "idl-build")]
struct IdlBuildIx {
    name: syn::Ident,
    disc: Vec<u8>,
    ctx_struct: syn::Path,
    arg_names: Vec<syn::Ident>,
    arg_types: Vec<syn::Type>,
    return_ty: Option<syn::Type>,
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
fn idl_rule_errors() -> proc_macro2::TokenStream {
    let modules = naclac_syn::modules::crate_modules(&crate::manifest_dir());
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

/// Builds the whole `idl-build` assembler — a plain `pub fn __naclac_print_idl()`
/// at crate-root scope (never `#[test]`-tagged; a `[[bin]]` target's `main()`
/// can't invoke a `#[test]` function, confirmed against how Anchor's own
/// equivalent — which *does* rely on `#[test]` plus `cargo test`'s built-in
/// scattered-function discovery — doesn't transfer to naclac's `[[bin]]`
/// delivery choice) that a small, separate `[[bin]]` entry point (CLI-scaffolded,
/// not generated here — a proc-macro can't create files or edit `Cargo.toml`)
/// calls directly. Chases every reachable component type via each
/// instruction's `__naclac_insert_account_types`, resolves each instruction's
/// accounts (`__naclac_idl_accounts()`, patching in seed `field_type`/const
/// values from the assembled type map — see `idl/src/idl_build_accounts.rs`),
/// resolves instruction args locally and chases any that are themselves
/// `#[defined_type]`-tagged, reads `crate::ID` for the address, and — for
/// constants/events/errors, none of which any instruction signature ever
/// references directly — does a small compile-time-only scan
/// (`naclac_syn::names`) to find and call their own already-generated print
/// functions by their real module path. Gated on `idl-build` purely by this
/// function itself only existing under `#[cfg(feature = "idl-build")]` (see
/// the stub variant below) — not by any runtime env-var check.
/// `CARGO_FEATURE_IDL_BUILD` looked like it would work (real for build
/// scripts) but isn't actually visible to a proc-macro's own process at
/// all — confirmed empirically (zero `CARGO_FEATURE_*` vars present, not
/// even for features known active), not assumed.
#[cfg(feature = "idl-build")]
fn build_idl_assembler(ixs: &[IdlBuildIx]) -> proc_macro2::TokenStream {
    let idl = quote! { naclac_lang::naclac_idl };

    let handler_names: Vec<String> = ixs.iter().map(|ix| ix.name.to_string()).collect();
    let mut instruction_docs = naclac_syn::instruction_docs(&crate::manifest_dir(), &handler_names);

    let constants = crate::crate_constants();
    let aliases = crate::crate_aliases();
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
            let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                continue;
            };
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
                let return_json = serde_json::to_string(&naclac_syn::parser::rust_type_to_idl(
                    return_ty, &constants,
                ));
                if let Some(leaf_ty) = naclac_syn::parser::defined_type_leaf(return_ty) {
                    insert_defined_type_calls.push(quote! {
                        if let Some(ty) = <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::create_type() {
                            __defined_types.insert(<#leaf_ty as #idl::idl_build::NaclacIdlBuild>::get_full_path(), ty);
                            <#leaf_ty as #idl::idl_build::NaclacIdlBuild>::insert_types(&mut __defined_types);
                        }
                    });
                }
                match return_json {
                    Ok(json) => quote! {
                        Some(#idl::serde_json::from_str(#json)
                            .expect("naclac idl-build: generated return type JSON must parse"))
                    },
                    Err(_) => quote! { None },
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

    // Constants/events/errors are never referenced by any instruction
    // signature `#[program]` can see, so — only when `idl-build` is the
    // feature actually being compiled right now (never during a normal
    // build) — a small, name-and-module-path-only scan
    // (`naclac_syn::names::scan_tagged_items`) finds them, and calls to
    // each one's own already-generated print function are spliced in by
    // real path. See this function's own doc comment and
    // docs/plan/idl-build-compilation-migration.md for why this is the
    // right substitute for what Anchor gets for free from `#[test]` plus
    // `cargo test`'s built-in scattered-function discovery.
    let mut component_calls = Vec::new();
    let mut event_calls = Vec::new();
    let mut error_calls = Vec::new();
    let mut const_calls = Vec::new();
    let mut crate_const_pushes = Vec::new();
    // No `CARGO_FEATURE_IDL_BUILD`-style runtime check needed here — this
    // whole function only exists in the first place (see the `#[cfg(not(feature
    // = "idl-build"))]` stub variant below) when naclac-macros itself is
    // compiled with `idl-build` active, which normal Cargo feature
    // unification only does when something in the build graph actually
    // requests it. A plain `naclac build`/on-chain build never compiles
    // this code at all, let alone runs the scan — confirmed empirically:
    // `CARGO_FEATURE_*` env vars (a build-script-only Cargo convention, not
    // something a proc-macro's own process ever receives) are not a
    // reliable signal here, unlike this function-level `#[cfg(...)]` split.
    {
        let manifest_dir = crate::manifest_dir();
        let program_dir = manifest_dir.as_path();

        for item in naclac_syn::names::scan_tagged_items(program_dir, "component") {
            let Ok(path) =
                syn::parse_str::<syn::Path>(&format!("{}::{}", item.module_path, item.name))
            else {
                continue;
            };
            let cfgs = &item.cfgs;
            component_calls.push(if item.visible_from_root {
                quote! {
                    #(#cfgs)*
                    __naclac_insert_component::<#path>(
                        &mut __account_types,
                        &mut __account_discs,
                        &mut __defined_types,
                    );
                }
            } else {
                unreachable_item_error("component", &item)
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
            let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                continue;
            };
            let name = item.name;
            let docs = item.docs;
            let value = crate::constant_value_tokens(&item.ty, &quote! { #path });
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
            // Lowercased to match `constant_idl_build_impl`'s own naming
            // (`macros/src/lib.rs`) — a bare-uppercase function name
            // would trip `non_snake_case` on the generated definition.
            let Ok(fn_path) = syn::parse_str::<syn::Path>(&format!(
                "{}::__naclac_idl_print_const_{}",
                item.module_path,
                item.name.to_lowercase()
            )) else {
                continue;
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

            fn __naclac_insert_component<T: #idl::idl_build::NaclacIdlBuild>(
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
            #(#component_calls)*
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
struct IdlBuildIx;

#[cfg(not(feature = "idl-build"))]
fn build_idl_assembler(_ixs: &[IdlBuildIx]) -> proc_macro2::TokenStream {
    quote! {}
}

/// Extracts `T` from a handler's `-> Result<T>` return type. Bare `Result`
/// (no `<...>` at all, i.e. `T` defaulted to `()`) and an explicit
/// `Result<()>` both mean "no return value" and yield `None`; anything else
/// yields `Some(T)`. Mirrors Anchor v2's own `extract_result_return_type`.
fn extract_result_return_type(output: &syn::ReturnType) -> Option<Type> {
    let syn::ReturnType::Type(_, ty) = output else {
        return None;
    };
    let Type::Path(type_path) = &**ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        // Bare `Result`, no generics written at all — defaults to T = ().
        return None;
    };
    let syn::GenericArgument::Type(return_ty) = args.args.first()? else {
        return None;
    };
    if is_unit_type(return_ty) {
        None
    } else {
        Some(return_ty.clone())
    }
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}

/// If `ty` is literally `Option<Inner>`, returns `Inner`.
fn extract_option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };
    Some(inner)
}

/// Generates the code that serializes `__result`'s `Ok` value (if the
/// handler declared one via `-> Result<T>`) and sets it as the instruction's
/// return data — via `NaclacReturnData::to_return_data`, generated by
/// `#[component]`/`#[event]`/the blanket `Pod` impl. `Option<T>` is detected
/// here, at macro-expansion time, rather than via a blanket `impl<T> for
/// Option<T>` in `naclac-core`, which would conflict with the blanket
/// `impl<T: Pod> for T` under Rust's coherence rules.
fn set_return_data_tokens(
    return_ty: &Option<Type>,
    set_return_data_call: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let Some(return_ty) = return_ty else {
        return quote! {};
    };
    if let Some(_inner) = extract_option_inner(return_ty) {
        quote! {
            if let Ok(__value) = &__result {
                match __value {
                    Some(__inner) => {
                        let __encoded = naclac_lang::prelude::NaclacReturnData::to_return_data(__inner);
                        let mut __return_data = naclac_lang::prelude::Vec::with_capacity(1 + __encoded.len());
                        __return_data.push(1u8);
                        __return_data.extend_from_slice(&__encoded);
                        #set_return_data_call(&__return_data);
                    }
                    None => {
                        #set_return_data_call(&[0u8]);
                    }
                }
            }
        }
    } else {
        quote! {
            if let Ok(__value) = &__result {
                let __return_data = naclac_lang::prelude::NaclacReturnData::to_return_data(__value);
                #set_return_data_call(&__return_data);
            }
        }
    }
}

/// How the program crate's root file declares `no_std`. An attribute macro
/// cannot see its crate's inner attributes, so `crate_root_std` reads them
/// from the crate root file itself.
enum CrateStd {
    Std,
    NoStd,
    /// `#![cfg_attr(<predicate>, no_std)]`: `no_std` exactly when the
    /// predicate holds.
    NoStdWhen(proc_macro2::TokenStream),
}

/// Reads the program crate's root file (`[lib] path` in its `Cargo.toml`,
/// else `src/lib.rs`) and reports how it declares `no_std`. An error means
/// the file could not be found or parsed.
fn crate_root_std() -> Result<CrateStd, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_string())?;
    let root = naclac_syn::modules::crate_root(std::path::Path::new(&manifest_dir));
    let source = std::fs::read_to_string(&root)
        .map_err(|error| format!("cannot read the crate root {}: {error}", root.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("cannot parse the crate root {}: {error}", root.display()))?;

    for attr in &file.attrs {
        if !matches!(attr.style, syn::AttrStyle::Inner(_)) {
            continue;
        }
        if attr.path().is_ident("no_std") {
            return Ok(CrateStd::NoStd);
        }
        if attr.path().is_ident("cfg_attr") {
            let Ok(args) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) else {
                continue;
            };
            let mut args = args.into_iter();
            let Some(predicate) = args.next() else {
                continue;
            };
            if args.any(|meta| meta.path().is_ident("no_std")) {
                return Ok(CrateStd::NoStdWhen(quote! { #predicate }));
            }
        }
    }
    Ok(CrateStd::Std)
}

/// The program's panic support for the Solana target, matched to how its
/// crate declares `no_std`: the `custom_panic` hook for a `std` crate, the
/// `no_std` choice (`__naclac_nostd_panic!`) for a `no_std` one, and both
/// behind the crate's own predicate for a `cfg_attr` one. Omitted under
/// `no-entrypoint`, where the program that links this one supplies it.
fn panic_support_tokens() -> proc_macro2::TokenStream {
    let no_entrypoint = quote! { #[cfg(not(feature = "no-entrypoint"))] };
    match crate_root_std() {
        Ok(CrateStd::Std) => quote! {
            #no_entrypoint
            naclac_lang::__naclac_panic_hook!();
        },
        Ok(CrateStd::NoStd) => quote! {
            #no_entrypoint
            naclac_lang::__naclac_nostd_panic!();
        },
        Ok(CrateStd::NoStdWhen(predicate)) => quote! {
            #no_entrypoint
            #[cfg(#predicate)]
            naclac_lang::__naclac_nostd_panic!();
            #no_entrypoint
            #[cfg(not(#predicate))]
            naclac_lang::__naclac_panic_hook!();
        },
        Err(message) => {
            let message = format!(
                "Naclac Error: #[program] could not determine whether this crate is `no_std`: {message}"
            );
            quote! {
                #[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
                compile_error!(#message);
            }
        }
    }
}

/// Tokens evaluating to the leading `width`-byte instruction discriminator as
/// an integer, for `instruction_data` already checked to hold at least `width`
/// bytes. Widths 1, 2, 4 and 8 read one native integer; any other width is
/// zero-padded into a `u64`. Must produce the same integer type and value
/// as `disc_arm_literal` builds for the same width.
fn disc_read_tokens(width: usize) -> proc_macro2::TokenStream {
    match width {
        1 => quote! { instruction_data[0] },
        2 => quote! { u16::from_le_bytes([instruction_data[0], instruction_data[1]]) },
        4 => quote! {
            u32::from_le_bytes([
                instruction_data[0],
                instruction_data[1],
                instruction_data[2],
                instruction_data[3],
            ])
        },
        8 => quote! {
            u64::from_le_bytes(unsafe { *(instruction_data.as_ptr() as *const [u8; 8]) })
        },
        n => quote! {{
            let mut __disc_buf = [0u8; 8];
            __disc_buf[..#n].copy_from_slice(&instruction_data[..#n]);
            u64::from_le_bytes(__disc_buf)
        }},
    }
}

/// The match-arm pattern for a discriminator of `width` bytes, with the same
/// integer type and byte order as `disc_read_tokens(width)`.
fn disc_arm_literal(width: usize, bytes: &[u8]) -> proc_macro2::TokenStream {
    match width {
        1 => {
            let value = bytes[0];
            quote! { #value }
        }
        2 => {
            let value = u16::from_le_bytes([bytes[0], bytes[1]]);
            quote! { #value }
        }
        4 => {
            let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            quote! { #value }
        }
        _ => {
            let mut padded = [0u8; 8];
            padded[..width].copy_from_slice(bytes);
            let value = u64::from_le_bytes(padded);
            quote! { #value }
        }
    }
}

/// A helper function to traverse the AST and extract the underlying accounts struct
/// from `Context<MyAccounts>` or `&mut Context<MyAccounts>`.
fn extract_context_struct(ty: &Type) -> Option<syn::Path> {
    // Support both `Context<T>` and `&mut Context<T>`
    let ty_path = match ty {
        Type::Path(p) => Some(p),
        Type::Reference(r) => {
            if let Type::Path(p) = &*r.elem {
                Some(p)
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(type_path) = ty_path {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Context" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        // Ignore lifetimes, find the actual Type
                        if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                            let mut path = inner_path.path.clone();
                            // Strip any generic arguments (like <'info>) to avoid syntax errors when used in expressions
                            if let Some(last_segment) = path.segments.last_mut() {
                                last_segment.arguments = syn::PathArguments::None;
                            }
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Expands the `#[program]` macro.
///
/// This macro traverses the module items, finds public functions representing instructions,
/// and generates a highly optimized `match` dispatcher. It handles routing and signature
/// rewrites for the program's layout and runtime:
/// - **`borsh`** (`solana` runtime): Emits `BorshDeserialize` logic.
/// - **`wincode`**: Emits Wincode argument decoding, on either runtime.
/// - **`bytemuck`**: Emits native zero-copy slicing logic, on either runtime.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = parse_macro_input!(item as ItemMod);
    let mod_name = &module.ident;
    let mod_vis = &module.vis;

    let (declared_address, declaring_file) =
        match crate::instruction::security::declared_program_id() {
            Ok(declared) => declared,
            Err(message) => {
                return syn::Error::new_spanned(&module.ident, format!("Naclac Error: {message}"))
                    .to_compile_error()
                    .into();
            }
        };

    let content_items = match &module.content {
        Some((_, items)) => items.clone(),
        None => panic!("Naclac: #[program] requires a module body with items."),
    };

    // Each dispatch arm as `(pattern, body)`; `body` evaluates to a
    // `ProgramResult` and may `return` one early.
    let mut dispatch_arms: Vec<(proc_macro2::TokenStream, proc_macro2::TokenStream)> = Vec::new();
    #[cfg(feature = "idl-build")]
    let mut idl_build_ixs: Vec<IdlBuildIx> = Vec::new();
    #[cfg(not(feature = "idl-build"))]
    let idl_build_ixs: Vec<IdlBuildIx> = Vec::new();

    // 1. Iterate through all items in the module to find the developer's instructions
    let mut processed_content_items = Vec::new();

    // `#[program]` takes no argument at all — enforced by `lib.rs`'s
    // `reject_nonempty_attr` before this function is ever called.
    //
    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `component.rs`'s doc comment for the full
    // story), so per-argument deserialization below emits both
    // representations, each gated by a real `#[cfg(...)]` in the output
    // that the calling crate's own compiler resolves.
    let zero_copy_cfg = quote! { #[cfg(feature = "bytemuck")] };
    let borsh_cfg = quote! { #[cfg(feature = "borsh")] };
    let wincode_cfg = quote! { #[cfg(feature = "wincode")] };

    let mut ix_disc_by_name: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    let mut ix_disc_entries: Vec<naclac_syn::discriminator::DiscriminatorEntry> = Vec::new();
    for item in &content_items {
        let Item::Fn(func) = item else { continue };
        if !matches!(func.vis, syn::Visibility::Public(_)) {
            continue;
        }
        let name = func.sig.ident.to_string();
        let override_bytes = match naclac_syn::discriminator::parse_handler_override(&func.attrs) {
            Ok(bytes) => bytes,
            Err(err) => return err.to_compile_error().into(),
        };
        let bytes = naclac_syn::discriminator::resolve_discriminator(
            naclac_syn::discriminator::DiscriminatorKind::Instruction,
            &name,
            override_bytes.as_deref(),
        );
        ix_disc_by_name.insert(name.clone(), bytes.clone());
        ix_disc_entries.push(naclac_syn::discriminator::DiscriminatorEntry { name, bytes });
    }
    let ix_disc_len = ix_disc_entries.first().map_or(
        naclac_syn::discriminator::DEFAULT_DISCRIMINATOR_LEN,
        |entry| entry.bytes.len(),
    );

    let mut discriminator_errors: Vec<String> = Vec::new();
    if let Err(errs) = naclac_syn::discriminator::validate_category(
        naclac_syn::discriminator::DiscriminatorKind::Instruction,
        &ix_disc_entries,
    ) {
        discriminator_errors.extend(errs);
    }
    {
        let found = naclac_syn::discriminator::scan_crate_discriminators(&crate::manifest_dir());
        for (kind, entries) in [
            (
                naclac_syn::discriminator::DiscriminatorKind::Account,
                &found.accounts,
            ),
            (
                naclac_syn::discriminator::DiscriminatorKind::Event,
                &found.events,
            ),
        ] {
            if let Err(errs) = naclac_syn::discriminator::validate_category(kind, entries) {
                discriminator_errors.extend(errs);
            }
        }
    }
    if !discriminator_errors.is_empty() {
        let errors = discriminator_errors
            .iter()
            .map(|msg| syn::Error::new(proc_macro2::Span::call_site(), msg).to_compile_error());
        return quote! { #(#errors)* }.into();
    }

    let mut event_cpi_ctx_structs: Vec<syn::Path> = Vec::new();

    for item in &content_items {
        if let Item::Fn(func) = item {
            let mut new_func = func.clone();

            // Only process public functions (these are the instructions)
            if matches!(func.vis, syn::Visibility::Public(_)) {
                let func_name = &func.sig.ident;
                new_func
                    .attrs
                    .retain(|attr| !attr.path().is_ident("discriminator"));

                // Arguments explicitly marked `#[allow_heap]` skip the heap-collection
                // warning below entirely — a purpose-built, macro-native opt-out rather
                // than rustc's generic `#[allow(deprecated)]`, which would also silence
                // unrelated real deprecation warnings anywhere else in the same function.
                // The marker is consumed here (stripped from `pat_type.attrs` before
                // `new_func` is re-emitted) since it isn't a real attribute rustc knows.
                let mut heap_allowed_args: std::collections::HashSet<syn::Ident> =
                    std::collections::HashSet::new();

                // --- SIGNATURE REWRITING (AUTOMATIC LIFETIMES & REFERENCES) ---
                for arg in &mut new_func.sig.inputs {
                    if let FnArg::Typed(pat_type) = arg {
                        let had_allow_heap = pat_type
                            .attrs
                            .iter()
                            .any(|a| a.path().is_ident("allow_heap"));
                        pat_type.attrs.retain(|a| !a.path().is_ident("allow_heap"));
                        if had_allow_heap {
                            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                                heap_allowed_args.insert(pat_ident.ident.clone());
                            }
                        }

                        // Re-qualify a bare `ZcVec<T>`/`ZcString` to its fully-qualified
                        // prelude path. Always safe regardless of representation —
                        // this only changes spelling, never the type the signature
                        // resolves to.
                        let ty = &pat_type.ty;
                        if crate::type_classify::is_exactly(ty, "ZcVec") {
                            let new_ty: syn::Type =
                                match crate::type_classify::first_generic_type(ty) {
                                    Some(inner) => {
                                        syn::parse_quote! { naclac_lang::prelude::ZcVec<#inner> }
                                    }
                                    None => syn::parse_quote! { naclac_lang::prelude::ZcVec<u8> },
                                };
                            *pat_type.ty = new_ty;
                        } else if crate::type_classify::is_exactly(ty, "ZcString") {
                            let new_ty: syn::Type =
                                syn::parse_str("naclac_lang::prelude::ZcString").unwrap();
                            *pat_type.ty = new_ty;
                        }
                        // Standard Vec<T> and String: leave as-is, they use heap allocation.

                        // Strip any references to get to the underlying Context type
                        let mut inner_ty = (*pat_type.ty).clone();

                        if let Type::Reference(r) = &inner_ty {
                            inner_ty = (*r.elem).clone();
                        }

                        if let Type::Path(type_path) = &mut inner_ty {
                            if let Some(segment) = type_path.path.segments.last_mut() {
                                if segment.ident == "Context" {
                                    if let syn::PathArguments::AngleBracketed(args) =
                                        &mut segment.arguments
                                    {
                                        // Find the account struct type arg
                                        let mut type_arg_idx = None;
                                        for (i, arg) in args.args.iter().enumerate() {
                                            if let syn::GenericArgument::Type(_) = arg {
                                                type_arg_idx = Some(i);
                                                break;
                                            }
                                        }

                                        if let Some(idx) = type_arg_idx {
                                            if let syn::GenericArgument::Type(Type::Path(
                                                inner_path,
                                            )) = &mut args.args[idx]
                                            {
                                                if let Some(last_seg) =
                                                    inner_path.path.segments.last_mut()
                                                {
                                                    last_seg.arguments = syn::PathArguments::None;
                                                }
                                            }

                                            // Ensure Context has precisely <T> (lifetime-free)
                                            let mut new_args = syn::punctuated::Punctuated::<
                                                syn::GenericArgument,
                                                syn::Token![,],
                                            >::new(
                                            );
                                            new_args.push(args.args[idx].clone());
                                            args.args = new_args;
                                        }
                                    }

                                    // Standardize the parameter type to Context<T>
                                    // avoiding &mut references to prevent type mismatches in CPI wrappers.
                                    *pat_type.ty = syn::parse_quote! { #inner_ty };
                                }
                            }
                        }
                    }
                }

                let ix_disc_bytes = ix_disc_by_name
                    .get(&func_name.to_string())
                    .expect("every public handler is resolved by the discriminator pre-pass")
                    .clone();

                // --- SIGNATURE PARSING ---
                let mut context_struct_name = None;
                let mut arg_names = Vec::new();
                let mut arg_types = Vec::new();

                for arg in &func.sig.inputs {
                    if let FnArg::Typed(pat_type) = arg {
                        if let Pat::Ident(pat_ident) = &*pat_type.pat {
                            let arg_name = &pat_ident.ident;
                            if let Some(ctx_path) = extract_context_struct(&pat_type.ty) {
                                context_struct_name = Some(ctx_path);
                            } else {
                                arg_names.push(arg_name);
                                arg_types.push(&pat_type.ty);
                            }
                        }
                    }
                }

                let ctx_struct = match context_struct_name {
                    Some(name) => name,
                    None => panic!(
                        "Naclac: Instruction '{}' must have a Context argument.",
                        func_name
                    ),
                };

                // --- ARGUMENT DESERIALIZATION LOGIC ---
                let mut deserialization_logic = Vec::new();
                let mut arg_tuple_names = Vec::new();
                let mut arg_tuple_types = Vec::new();
                let mut slice_arg_names = Vec::new();

                // __ix_offset is emitted conditionally — only when there are non-slice args.
                // Instructions with no arguments (e.g. increment) get zero dead code.
                let mut has_non_slice_args = false;

                for (name, ty) in arg_names.iter().zip(arg_types.iter()) {
                    if crate::type_classify::is_byte_slice_ref(ty) {
                        slice_arg_names.push(*name);
                        continue;
                    }

                    arg_tuple_names.push(*name);
                    arg_tuple_types.push(*ty);
                    has_non_slice_args = true;

                    use crate::type_classify::DynamicKind;
                    let kind = crate::type_classify::classify_dynamic(ty);
                    let is_zero_copy_vec = matches!(kind, DynamicKind::ZcVec);
                    let is_zero_copy_string = matches!(kind, DynamicKind::ZcString);
                    let is_zero_copy_collection = is_zero_copy_vec || is_zero_copy_string;
                    let is_heap_vec = matches!(kind, DynamicKind::HeapVec);
                    let is_heap_string = matches!(kind, DynamicKind::HeapString);
                    let is_heap_collection = is_heap_vec || is_heap_string;
                    // `ty` isn't itself literally `ZcVec`/`ZcString` (that's
                    // `is_zero_copy_collection` above) but may still be an
                    // opaque struct name (e.g. an `#[instruction_args]` type)
                    // that *contains* one — same unsound-in-Borsh-mode case,
                    // just one level removed. See `dynamic_view_field_of`'s
                    // own doc comment for why this needs a real source scan
                    // rather than being visible from `ty` alone.
                    let opaque_zc_field = crate::type_classify::base_ident(ty).and_then(|id| {
                        crate::instruction::security::dynamic_view_field_of(&id.to_string())
                    });

                    // Cargo gives a proc-macro no reliable way to see the invoking
                    // crate's activated features (see `component.rs`'s doc comment
                    // for the full story), so both representations are always
                    // computed below and spliced in together, each gated by a real
                    // `#[cfg(...)]` in the output the calling crate's own compiler
                    // resolves. Which of `zero_copy_stmt`'s two shapes applies is a
                    // separate, purely structural question (does this arg's own
                    // declared type contain a dynamic collection or not) — safe to
                    // decide with an ordinary Rust `if` here, no feature detection
                    // involved either way.
                    let zero_copy_stmt = if !is_zero_copy_collection && !is_heap_collection {
                        // `NaclacArgs` (not `NaclacPod` directly) — covers both a
                        // plain fixed-size arg (via `NaclacPod`'s blanket
                        // `NaclacArgs` impl) and an `#[instruction_args]`-grouped
                        // struct containing a `ZcString`/`ZcVec` field, whose
                        // encoded length depends on its own content and can
                        // never be read via a single raw `size_of`-based cast.
                        quote! {
                            let #name = <#ty as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                                instruction_data, &mut __ix_offset
                            )?;
                        }
                    } else {
                        let inner_ty = crate::type_classify::first_generic_type(ty);

                        let __v_logic = if is_zero_copy_collection {
                            if is_zero_copy_vec {
                                if let Some(inner) = &inner_ty {
                                    quote! {
                                        let __v = <naclac_lang::prelude::ZcVec<#inner>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                        __ix_offset += __len;
                                        __v
                                    }
                                } else {
                                    quote! {
                                        let __v = <naclac_lang::prelude::ZcVec<u8>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                        __ix_offset += __len;
                                        __v
                                    }
                                }
                            } else {
                                quote! {
                                    let __v = <naclac_lang::prelude::ZcString>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                    __ix_offset += __len;
                                    __v
                                }
                            }
                        } else {
                            let is_nested_vec = is_heap_vec
                                && inner_ty.is_some_and(|inner| {
                                    crate::type_classify::is_exactly(inner, "Vec")
                                        || crate::type_classify::is_exactly(inner, "String")
                                });
                            if is_heap_string {
                                quote! {
                                    let __bytes_len = __len as usize;
                                    if instruction_data.len() < __ix_offset + __bytes_len {
                                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                    }
                                    let __v = naclac_lang::prelude::String::from_utf8(instruction_data[__ix_offset..__ix_offset+__bytes_len].to_vec())
                                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                                    __ix_offset += __bytes_len;
                                    __v
                                }
                            } else if is_nested_vec {
                                quote! {
                                    naclac_lang::prelude::read_length_prefixed_items(
                                        instruction_data,
                                        &mut __ix_offset,
                                        __len,
                                    )?
                                }
                            } else {
                                if let Some(inner) = &inner_ty {
                                    if crate::type_classify::is_exactly(inner, "u8") {
                                        quote! {
                                            let __bytes_len = __len as usize;
                                            if instruction_data.len() < __ix_offset + __bytes_len {
                                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                            }
                                            let mut __v = naclac_lang::prelude::Vec::with_capacity(__bytes_len);
                                            __v.extend_from_slice(&instruction_data[__ix_offset..__ix_offset+__bytes_len]);
                                            __ix_offset += __bytes_len;
                                            __v
                                        }
                                    } else {
                                        quote! {
                                            let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                                            if __sz == 0
                                                || __len % __sz != 0
                                                || instruction_data.len() < __ix_offset + __len
                                            {
                                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                            }
                                            let mut __v = naclac_lang::prelude::Vec::with_capacity(__len / __sz);
                                            let __end = __ix_offset + __len;
                                            while __ix_offset < __end {
                                                __v.push(
                                                    <#inner as naclac_lang::prelude::NaclacPod>::naclac_try_from_bytes(
                                                        &instruction_data[__ix_offset..__ix_offset + __sz]
                                                    )
                                                    .ok_or_else(|| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?,
                                                );
                                                __ix_offset += __sz;
                                            }
                                            __v
                                        }
                                    }
                                } else {
                                    quote! {
                                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                    }
                                }
                            }
                        };

                        quote! {
                            let #name = {
                                if instruction_data.len() < __ix_offset + 4 {
                                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                }
                                let mut __len_bytes = [0u8; 4];
                                __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                                let __len = u32::from_le_bytes(__len_bytes) as usize;
                                __ix_offset += 4;
                                #__v_logic
                            };
                        }
                    };

                    // Real Borsh already decodes `Vec<T>`/`String` natively via its
                    // own derive, so — unlike the zero-copy side — one uniform
                    // BorshDeserialize call correctly covers every fixed or
                    // heap-collection arg shape here. `ZcVec<T>`/
                    // `ZcString` are the one exception: zero-copy-only *views*
                    // (pointer + length) with no bytes of their own to serialize,
                    // so calling BorshDeserialize on them is unsound-by-
                    // construction, not just unsupported — a clear, purpose-built
                    // error takes its place instead of the confusing raw
                    // trait-bound error rustc would otherwise give (mirrors
                    // `#[instruction_args]`'s identical `zc_field` check in
                    // macros/src/lib.rs). `compile_error!` only queues a
                    // diagnostic — it doesn't stop rustc from still
                    // type-checking the rest of this function, and `#name` is
                    // referenced again later (unconditionally, by
                    // `arg_injections`) when calling the actual handler. Still
                    // binding `#name` (to a `!`-typed, never-executed
                    // `unreachable!()`) keeps that later reference valid, so
                    // this compile_error! stays the only thing the caller sees
                    // for this argument, not a second, unrelated "cannot find
                    // value" cascade on top of it.
                    let borsh_stmt = if is_zero_copy_collection {
                        let found_ty = if is_zero_copy_vec {
                            "ZcVec<T>"
                        } else {
                            "ZcString"
                        };
                        let msg = format!(
                            "Naclac Error: argument '{}' uses a zero-copy-only type ('{}') in a \
                             Borsh-mode instruction. These are views (pointer + length) into the \
                             current instruction's byte buffer with no bytes of their own to \
                             serialize — real Borsh already decodes 'Vec<T>'/'String' natively, so \
                             use one of those instead.",
                            name, found_ty
                        );
                        quote! {
                            #[allow(unreachable_code)]
                            let #name: #ty = { compile_error!(#msg); unreachable!() };
                        }
                    } else if let Some(field_name) = &opaque_zc_field {
                        let msg = format!(
                            "Naclac Error: argument '{}' has type '{}', whose own field '{}' uses a \
                             zero-copy-only type ('ZcVec<T>'/'ZcString') in a Borsh-mode \
                             instruction. These are views (pointer + length) into the current \
                             instruction's byte buffer with no bytes of their own to serialize — real \
                             Borsh already decodes 'Vec<T>'/'String' natively, so use one of those \
                             instead of '{}' in '{}'.",
                            name,
                            quote! { #ty },
                            field_name,
                            field_name,
                            quote! { #ty },
                        );
                        quote! {
                            #[allow(unreachable_code)]
                            let #name: #ty = { compile_error!(#msg); unreachable!() };
                        }
                    } else {
                        quote! {
                            let #name = {
                                let mut __r: &[u8] = &instruction_data[__ix_offset..];
                                let __val = <#ty as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                                __ix_offset = instruction_data.len() - __r.len();
                                __val
                            };
                        }
                    };

                    // Only meaningful for the zero-copy side — Borsh mode has no
                    // ZcVec/ZcString alternative to nudge toward.
                    let mut arg_stmt = quote! {};
                    if is_heap_collection && !heap_allowed_args.contains(*name) {
                        let found_ty = if is_heap_vec { "Vec<T>" } else { "String" };
                        let suggested_ty = if is_heap_vec { "ZcVec<T>" } else { "ZcString" };
                        let warning = crate::heap_collection_warning(
                            &name.to_string(),
                            found_ty,
                            suggested_ty,
                        );
                        arg_stmt = quote! { #arg_stmt #zero_copy_cfg { #warning } };
                    }
                    let wincode_stmt = if is_zero_copy_collection {
                        let found_ty = if is_zero_copy_vec {
                            "ZcVec<T>"
                        } else {
                            "ZcString"
                        };
                        let msg = format!(
                            "Naclac Error: argument '{}' uses a zero-copy-only type ('{}') in a \
                             Wincode-mode instruction. These are views (pointer + length) into the \
                             current instruction's byte buffer with no bytes of their own to \
                             serialize — use 'Vec<T>'/'String' instead.",
                            name, found_ty
                        );
                        quote! {
                            #[allow(unreachable_code)]
                            let #name: #ty = { compile_error!(#msg); unreachable!() };
                        }
                    } else if let Some(field_name) = &opaque_zc_field {
                        let msg = format!(
                            "Naclac Error: argument '{}' has type '{}', whose own field '{}' uses a \
                             zero-copy-only type ('ZcVec<T>'/'ZcString') in a Wincode-mode \
                             instruction. These are views (pointer + length) into the current \
                             instruction's byte buffer with no bytes of their own to serialize — use \
                             'Vec<T>'/'String' instead of '{}' in '{}'.",
                            name,
                            quote! { #ty },
                            field_name,
                            field_name,
                            quote! { #ty },
                        );
                        quote! {
                            #[allow(unreachable_code)]
                            let #name: #ty = { compile_error!(#msg); unreachable!() };
                        }
                    } else {
                        quote! {
                            let #name = {
                                let mut __r: &[u8] = &instruction_data[__ix_offset..];
                                let __val: #ty = naclac_lang::prelude::wincode::config::deserialize_from(&mut __r, naclac_lang::prelude::wincode_config::CONFIG)
                                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                                __ix_offset = instruction_data.len() - __r.len();
                                __val
                            };
                        }
                    };

                    deserialization_logic.push(quote! {
                        #arg_stmt
                        #zero_copy_cfg
                        #zero_copy_stmt
                        #borsh_cfg
                        #borsh_stmt
                        #wincode_cfg
                        #wincode_stmt
                    });
                }

                // Build the final deserialization block, prepending __ix_offset init
                // only when at least one non-slice argument was found.
                let _args_deserialization = if has_non_slice_args {
                    quote! {
                        let mut __ix_offset: usize = #ix_disc_len;
                        #( #deserialization_logic )*
                    }
                } else {
                    quote! { #( #deserialization_logic )* }
                };

                // Args are now local variables — reference directly (no __args.name)
                let arg_injections = arg_names.iter().map(|name| {
                    if slice_arg_names.contains(name) {
                        quote! { , &instruction_data[#ix_disc_len..] }
                    } else {
                        quote! { , #name }
                    }
                });
                let arg_injections = quote! { #(#arg_injections)* };

                // --- THE DISPATCH FUNCTION ---
                let dispatch_func_name = format_ident!("__dispatch_{}", func_name);

                let return_ty = extract_result_return_type(&func.sig.output);
                let set_return_data_pinocchio = set_return_data_tokens(
                    &return_ty,
                    quote! { naclac_lang::prelude::pinocchio::cpi::set_return_data },
                );
                let set_return_data_solana = set_return_data_tokens(
                    &return_ty,
                    quote! { naclac_lang::cpi::sdk::set_return_data },
                );

                // Pinocchio path: takes pre-walked &[AccountType] slice.
                // load_and_validate returns (accounts, remaining_slice, bumps).
                processed_content_items.push(syn::parse_quote! {
                    #[cfg(not(feature = "solana"))]
                    pub(crate) fn #dispatch_func_name(
                        program_id: &'static naclac_lang::prelude::Address,
                        __input: *mut u8,
                        __num_accounts: usize,
                        __lookup: &'static mut [core::mem::MaybeUninit<naclac_lang::prelude::AccountView>; 64],
                        instruction_data: &'static [u8],
                    ) -> naclac_lang::prelude::ProgramResult {
                        #_args_deserialization
                        // SAFETY: `__input`/`__num_accounts` come straight from the
                        // runtime entrypoint (see `process_instruction`).
                        let (mut accounts, __rem_slice, bumps) = unsafe {
                            #ctx_struct::load_and_validate::<#ix_disc_len>(
                                program_id,
                                __input,
                                __num_accounts,
                                __lookup,
                                instruction_data,
                            )
                        }?;
                        let ctx = unsafe {
                            naclac_lang::prelude::Context::new(
                                program_id,
                                &mut accounts,
                                __rem_slice,
                                bumps,
                            )
                        };
                        let __result = #mod_name::#func_name(ctx #arg_injections);
                        #set_return_data_pinocchio
                        if __result.is_ok() {
                            accounts.teardown(program_id, &bumps)?;
                        }
                        __result.map(|_| ())
                    }
                });

                // Standard Solana path: takes pre-built &[AccountInfo].
                processed_content_items.push(syn::parse_quote! {
                    #[cfg(feature = "solana")]
                    pub(crate) fn #dispatch_func_name(
                        program_id: &naclac_lang::prelude::Address,
                        accounts: &[naclac_lang::prelude::AccountInfo],
                        instruction_data: &[u8],
                        __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
                    ) -> naclac_lang::prelude::Result {
                        #_args_deserialization
                        let (mut accounts, remaining_accounts, bumps) = #ctx_struct::load_and_validate::<#ix_disc_len>(
                            program_id,
                            accounts,
                            instruction_data,
                            __duplicates,
                        )?;
                        let ctx = unsafe {
                            naclac_lang::prelude::Context::new(
                                program_id,
                                #ctx_struct::as_mut_payload(&mut accounts),
                                remaining_accounts,
                                bumps,
                            )
                        };
                        let __result = #mod_name::#func_name(ctx #arg_injections);
                        #set_return_data_solana
                        if __result.is_ok() {
                            #ctx_struct::teardown_payload(&mut accounts, program_id, &bumps)?;
                        }
                        __result.map(|_| ())
                    }
                });

                #[cfg(feature = "idl-build")]
                idl_build_ixs.push(IdlBuildIx {
                    name: func_name.clone(),
                    disc: ix_disc_bytes.clone(),
                    ctx_struct: ctx_struct.clone(),
                    arg_names: arg_names.iter().map(|n| (*n).clone()).collect(),
                    arg_types: arg_types.iter().map(|t| (***t).clone()).collect(),
                    return_ty: return_ty.clone(),
                });

                event_cpi_ctx_structs.push(ctx_struct.clone());

                let disc_literal = disc_arm_literal(ix_disc_len, &ix_disc_bytes);
                // --- THE MATCH ARM ---
                dispatch_arms.push((
                    quote! { #disc_literal },
                    quote! {
                        #[cfg(not(feature = "solana"))]
                        { #mod_name::#dispatch_func_name(program_id, input, num_accounts, __lookup, instruction_data) }
                        #[cfg(feature = "solana")]
                        { #mod_name::#dispatch_func_name(program_id, accounts, instruction_data, __dups_opt) }
                    },
                ));
            }
            processed_content_items.push(Item::Fn(new_func));
        } else {
            processed_content_items.push(item.clone());
        }
    }

    // --- CPI GENERATION (Moved to SDK) ---
    // The cpi module is now generated in the Rust SDK, not in the program crate.

    // --- EVENT-CPI RECEIVE ARM ---
    // Live only when some handler's accounts struct carries `#[event_cpi]`
    // (decided by a const-evaluated OR over `EventCpiAccounts::NEEDS_EVENT_CPI`
    // — no source scanning). The tag participates in dispatch exactly like a
    // real instruction discriminator of the same width.
    let event_tag_bytes = naclac_syn::discriminator::event_dispatch_tag(ix_disc_len);
    let colliding_handlers: Vec<&String> = ix_disc_by_name
        .iter()
        .filter(|(_, bytes)| **bytes == event_tag_bytes)
        .map(|(name, _)| name)
        .collect();
    processed_content_items.push(syn::parse_quote! {
        pub(super) const __NACLAC_NEEDS_EVENT_CPI: bool =
            false #(|| <#event_cpi_ctx_structs as naclac_lang::prelude::EventCpiAccounts>::NEEDS_EVENT_CPI)*;
    });
    let mut tagged_ctx_structs: Vec<&syn::Path> = Vec::new();
    for ctx_struct in &event_cpi_ctx_structs {
        let already_tagged = tagged_ctx_structs
            .iter()
            .any(|seen| quote!(#seen).to_string() == quote!(#ctx_struct).to_string());
        if !already_tagged {
            tagged_ctx_structs.push(ctx_struct);
        }
    }
    for ctx_struct in tagged_ctx_structs {
        processed_content_items.push(syn::parse_quote! {
            impl naclac_lang::prelude::EventCpiTag for #ctx_struct {
                const EVENT_DISPATCH_TAG: &'static [u8] = &[#(#event_tag_bytes),*];
            }
        });
    }
    if !colliding_handlers.is_empty() {
        let names = colliding_handlers
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let msg = format!(
            "Naclac Error: instruction(s) [{names}] use a discriminator equal to the reserved \
             event-CPI dispatch tag ({event_tag_bytes:?}), and this program uses `#[event_cpi]`. \
             Give the instruction a different `#[discriminator = ...]`."
        );
        processed_content_items.push(syn::parse_quote! {
            const _: () = {
                if __NACLAC_NEEDS_EVENT_CPI {
                    panic!(#msg);
                }
            };
        });
    }

    let event_authority_seed = b"__event_authority".to_vec();
    let (event_authority_pda, _bump) = crate::instruction::security::find_pda_at_compile_time(
        std::slice::from_ref(&event_authority_seed),
        &declared_address,
    )
    .unwrap_or_else(|| {
        panic!(
            "Naclac Error: the address `declare_id!` declares in {declaring_file} is not a valid \
             32-byte base58 address"
        )
    });
    let event_authority_bytes = bs58::decode(event_authority_pda)
        .into_vec()
        .expect("Invalid precomputed event authority PDA");
    let expected_authority = quote! {
        naclac_lang::prelude::Address::new_from_array([ #(#event_authority_bytes),* ])
    };
    let track_program_id = quote! { const _: &[u8] = include_bytes!(#declaring_file); };
    let event_tag_literal = disc_arm_literal(ix_disc_len, &event_tag_bytes);
    dispatch_arms.push((
        quote! { #event_tag_literal if #mod_name::__NACLAC_NEEDS_EVENT_CPI },
        quote! {
            #[cfg(not(feature = "solana"))]
            { __naclac_receive_event_cpi(input, num_accounts) }
            #[cfg(feature = "solana")]
            { __naclac_receive_event_cpi(accounts) }
        },
    ));
    let event_cpi_receive_fns = quote! {
        /// Handles an `emit_cpi!` self-invocation: succeeds only when the
        /// first account is the program's event authority PDA and signed.
        #[cfg(not(feature = "solana"))]
        #[inline(always)]
        fn __naclac_receive_event_cpi(input: *mut u8, num_accounts: usize) -> naclac_lang::prelude::ProgramResult {
            #track_program_id
            {
                if num_accounts == 0 {
                    return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                }
                // SAFETY: at least one account is present, it starts right after
                // the 8-byte account count, and the first serialized account is
                // never a duplicate entry.
                let __ea = unsafe {
                    naclac_lang::prelude::AccountView::new_unchecked(
                        input.add(8) as *mut naclac_lang::pinocchio::account::RuntimeAccount,
                    )
                };
                if !__ea.is_signer() {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }
                if naclac_lang::prelude::Address::from_address_ref(__ea.address()) != &#expected_authority {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
                }
                Ok(())
            }
        }

        /// Handles an `emit_cpi!` self-invocation: succeeds only when the
        /// first account is the program's event authority PDA and signed.
        #[cfg(feature = "solana")]
        #[inline(always)]
        fn __naclac_receive_event_cpi(accounts: &[naclac_lang::prelude::AccountType]) -> naclac_lang::prelude::Result {
            #track_program_id
            {
                let __ea_slice: &[naclac_lang::prelude::AccountType] = accounts;
                let Some(__ea) = __ea_slice.first() else {
                    return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                };
                if !__ea.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }
                if __ea.address() != #expected_authority {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
                }
                Ok(())
            }
        }
    };

    let match_arms: Vec<proc_macro2::TokenStream> = dispatch_arms
        .iter()
        .map(|(pattern, body)| quote! { #pattern => { #body } })
        .collect();
    let direct_return_arms: Vec<proc_macro2::TokenStream> = dispatch_arms
        .iter()
        .map(|(pattern, body)| {
            quote! {
                #pattern => naclac_lang::prelude::program_result_to_u64({ #body })
            }
        })
        .collect();

    let idl_assembler_tokens = build_idl_assembler(&idl_build_ixs);
    let disc_read = disc_read_tokens(ix_disc_len);
    let panic_support = panic_support_tokens();

    let idl_rule_errors = idl_rule_errors();

    // --- FINAL MACRO EXPANSION ---
    let expanded = quote! {
        #idl_rule_errors

        #[cfg(not(any(feature = "bytemuck", feature = "borsh", feature = "wincode")))]
        compile_error!(
            "naclac: this program declares no account-data layout. Add one of the `bytemuck`, \
             `borsh` or `wincode` features to its Cargo.toml, forwarded to `naclac-lang` \
             (for example `bytemuck = [\"naclac-lang/bytemuck\"]`), and enable it by default."
        );

        #[cfg(feature = "solana")]
        const _: bool = naclac_lang::__NACLAC_LANG_HAS_SOLANA_RUNTIME;
        #[cfg(not(feature = "solana"))]
        const _: bool = naclac_lang::__NACLAC_LANG_HAS_PINOCCHIO_RUNTIME;
        #[cfg(feature = "bytemuck")]
        const _: bool = naclac_lang::__NACLAC_LANG_HAS_BYTEMUCK_LAYOUT;
        #[cfg(feature = "borsh")]
        const _: bool = naclac_lang::__NACLAC_LANG_HAS_BORSH_LAYOUT;
        #[cfg(feature = "wincode")]
        const _: bool = naclac_lang::__NACLAC_LANG_HAS_WINCODE_LAYOUT;

        #[doc(hidden)]
        pub const __NACLAC_EVENT_DISPATCH_TAG: &[u8] = &[#(#event_tag_bytes),*];
        #[doc(hidden)]
        pub const __NACLAC_EVENT_CPI_IN_USE: bool = #mod_name::__NACLAC_NEEDS_EVENT_CPI;

        #[cfg(all(not(feature = "no-entrypoint"), feature = "solana"))]
        naclac_lang::naclac_solana_entrypoint!(process_instruction);

        #panic_support

        // Pinocchio SIMD-0321 entrypoint — two-parameter form.
        // The BPF loader passes r1=input (accounts), r2=ix_data_ptr (instruction data).
        // This eliminates parse_ix_context: we get ix_data directly without scanning accounts.
        #[cfg(all(not(feature = "no-entrypoint"), not(feature = "solana")))]
        #[no_mangle]
        pub unsafe extern "C" fn entrypoint(input: *mut u8, ix_data_ptr: *const u8) -> u64 {
            let num_accounts = unsafe { *(input as *const u64) } as usize;
            let len = unsafe { *(ix_data_ptr.sub(8) as *const u64) } as usize;
            // SAFETY: the loader places `len` instruction-data bytes at
            // `ix_data_ptr`, followed by the 32-byte program id.
            let (instruction_data, program_id): (&'static [u8], &'static naclac_lang::prelude::Address) = unsafe {
                (
                    core::slice::from_raw_parts(ix_data_ptr, len),
                    &*(ix_data_ptr.add(len) as *const naclac_lang::prelude::Address),
                )
            };
            __naclac_dispatch_to_u64(program_id, input, num_accounts, instruction_data)
        }

        /// Same dispatch as `process_instruction_inner`, but each arm converts
        /// its own result, so a successful arm returns `SUCCESS` directly.
        #[cfg(all(not(feature = "no-entrypoint"), not(feature = "solana")))]
        #[inline(always)]
        fn __naclac_dispatch_to_u64(
            program_id: &'static naclac_lang::prelude::Address,
            input: *mut u8,
            num_accounts: usize,
            instruction_data: &'static [u8],
        ) -> u64 {
            if instruction_data.len() < #ix_disc_len {
                return naclac_lang::prelude::program_result_to_u64(Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                ));
            }
            let instruction_discriminator = #disc_read;
            let mut __lookup_storage =
                [core::mem::MaybeUninit::<naclac_lang::prelude::AccountView>::uninit(); 64];
            // SAFETY: `__lookup_storage` lives in this frame, which outlives every
            // dispatch call below, and nothing else borrows it.
            let __lookup: &'static mut [core::mem::MaybeUninit<naclac_lang::prelude::AccountView>; 64] =
                unsafe { &mut *(&mut __lookup_storage as *mut _) };
            match instruction_discriminator {
                #(#direct_return_arms,)*
                _ => naclac_lang::prelude::program_result_to_u64(Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                )),
            }
        }

        #event_cpi_receive_fns
        #[cfg(all(not(feature = "no-entrypoint"), not(feature = "solana")))]
        naclac_lang::pinocchio::default_allocator!();

        // Unified Entrypoint — standard signature required by solana/pinocchio entrypoint! macro
        #[cfg(feature = "solana")]
        pub fn process_instruction(
            program_id: &naclac_lang::solana_address::Address,
            accounts: &[naclac_lang::solana_account_info::AccountInfo<'_>],
            instruction_data: &[u8],
        ) -> naclac_lang::prelude::Result {
            // SAFETY: The Solana runtime guarantees these references are valid for the
            // entire program invocation. We transmute the lifetime to 'static so the
            // inner dispatcher can safely bind them to Context<T> using 'static references.
            unsafe {
                let program_id: &'static naclac_lang::prelude::Address = core::mem::transmute(program_id);
                let accounts: &'static [naclac_lang::prelude::AccountInfoData] = core::mem::transmute(accounts);
                let instruction_data: &'static [u8] = core::mem::transmute(instruction_data);
                process_instruction_inner(
                    program_id,
                    naclac_lang::prelude::account_handles(accounts),
                    instruction_data,
                )
            }
        }

        // SIMD-0321: BPF loader passes r2 = pointer to instruction data bytes.
        // Layout: [u64 ix_data_len][ix_data bytes...][program_id: 32 bytes]
        // ix_data_ptr points to ix_data[0]; len at ix_data_ptr-8; program_id at ix_data_ptr+len.
        #[cfg(not(feature = "solana"))]
        /// Unified Entrypoint for Pinocchio — parses raw runtime inputs.
        ///
        /// # Safety
        ///
        /// This function is unsafe because it dereferences raw pointers (`input` and `ix_data_ptr`)
        /// supplied directly by the Solana runtime BPF loader. The caller must ensure these
        /// pointers point to valid, aligned, and initialized memory.
        pub unsafe fn process_instruction(input: *mut u8, ix_data_ptr: *const u8) -> naclac_lang::prelude::ProgramResult {
            // 1. Read num_accounts from the first 8 bytes.
            let num_accounts = unsafe { *(input as *const u64) } as usize;
            // 2. Decode instruction data directly from r2 — no account scan needed.
            let instruction_data: &'static [u8] = unsafe {
                let len = *(ix_data_ptr.sub(8) as *const u64) as usize;
                core::slice::from_raw_parts(ix_data_ptr, len)
            };
            // 3. Program ID immediately follows instruction data bytes.
            let program_id: &'static naclac_lang::prelude::Address = unsafe {
                let len = *(ix_data_ptr.sub(8) as *const u64) as usize;
                &*(ix_data_ptr.add(len) as *const naclac_lang::prelude::Address)
            };
            process_instruction_inner(program_id, input, num_accounts, instruction_data)
        }

        #[cfg(not(feature = "solana"))]
        #[inline(always)]
        fn process_instruction_inner(
            program_id: &'static naclac_lang::prelude::Address,
            input: *mut u8,
            num_accounts: usize,
            instruction_data: &'static [u8],
        ) -> naclac_lang::prelude::ProgramResult {
            if instruction_data.len() < #ix_disc_len {
                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
            }
            let instruction_discriminator = #disc_read;
            // Accounts are walked by the matched arm's own `load_and_validate`,
            // into this frame's lookup buffer.
            let mut __lookup_storage =
                [core::mem::MaybeUninit::<naclac_lang::prelude::AccountView>::uninit(); 64];
            // SAFETY: `__lookup_storage` lives in this frame, which outlives every
            // dispatch call below, and nothing else borrows it.
            let __lookup: &'static mut [core::mem::MaybeUninit<naclac_lang::prelude::AccountView>; 64] =
                unsafe { &mut *(&mut __lookup_storage as *mut _) };
            match instruction_discriminator {
                #(#match_arms),*
                _ => Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
            }
        }

        #[cfg(feature = "solana")]
        #[inline(always)]
        fn process_instruction_inner(
            program_id: &'static naclac_lang::prelude::Address,
            accounts: &'static [naclac_lang::prelude::AccountInfo],
            instruction_data: &'static [u8],
        ) -> naclac_lang::prelude::Result {
            if instruction_data.len() < #ix_disc_len {
                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
            }
            let instruction_discriminator = #disc_read;
            let mut __has_dups = false;
            let mut __dups = naclac_lang::prelude::AccountBitvec::default();
            // A repeat of the program's own address is an absent optional
            // account's sentinel, not an alias of a real account. Two entries
            // alias one account exactly when they share a key pointer: the
            // entrypoint clones the first entry for every loader-marked
            // duplicate, and the runtime never lists a key twice.
            for i in 0..accounts.len() {
                for j in 0..i {
                    if core::ptr::eq(accounts[i].key, accounts[j].key) && accounts[i].key != program_id {
                        __dups.set(i as u8);
                        __dups.set(j as u8);
                        __has_dups = true;
                    }
                }
            }
            let __dups_opt = if __has_dups { Some(&__dups) } else { None };
            match instruction_discriminator {
                #(#match_arms),*
                _ => Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
            }
        }



        #idl_assembler_tokens

        // Re-inject the developer's original module logic.
        #mod_vis mod #mod_name {
            use super::*;
            #(#processed_content_items)*
        }

    };

    expanded.into()
}
