//! # Program Macro Logic
//!
//! Handles the expansion of the `#[naclac_program]` macro, serving as the core router
//! and instruction dispatcher for the smart contract across all execution modes.

use naclac_cpi_core::{extract_automatic_instructions, generate_cpi_module};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use sha2::{Digest, Sha256};
use syn::{parse_macro_input, FnArg, Item, ItemMod, Pat, Type};

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
                                let ident_str = last_segment.ident.to_string();
                                if ident_str.ends_with("Hydrated") {
                                    last_segment.ident =
                                        format_ident!("{}", &ident_str[..ident_str.len() - 8]);
                                }
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

/// Expands the `#[naclac_program]` macro.
///
/// This macro traverses the module items, finds public functions representing instructions,
/// and generates a highly optimized `match` dispatcher. It handles routing and signature
/// rewrites across three distinct execution modes:
/// 1. **Solana Program + Borsh**: Emits `BorshDeserialize` logic.
/// 2. **Solana Program + Zero-Copy**: Emits native slicing logic alongside heap allocation for dynamic types.
/// 3. **Pinocchio + Zero-Copy**: Fully zero-copy slicing strictly for `no_std`.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = parse_macro_input!(item as ItemMod);
    let mod_name = &module.ident;
    let mod_vis = &module.vis;

    let content_items = match &module.content {
        Some((_, items)) => items.clone(),
        None => panic!("Naclac: #[naclac_program] requires a module body with items."),
    };

    let mut match_arms = Vec::new();

    // 1. Iterate through all items in the module to find the developer's instructions
    let mut processed_content_items = Vec::new();

    let attr_str = _attr.to_string();
    let is_program_zero_copy =
        attr_str.contains("zero_copy") || std::env::var("CARGO_FEATURE_PINOCCHIO").is_ok();

    for item in &content_items {
        if let Item::Fn(func) = item {
            let mut new_func = func.clone();

            // Only process public functions (these are the instructions)
            if matches!(func.vis, syn::Visibility::Public(_)) {
                let func_name = &func.sig.ident;

                // --- SIGNATURE REWRITING (AUTOMATIC LIFETIMES & REFERENCES) ---
                let mut has_context = false;
                for arg in &mut new_func.sig.inputs {
                    if let FnArg::Typed(pat_type) = arg {
                        // Detect if this is a Vec or String to perform type rewriting in Zero-Copy mode
                        let ty = &pat_type.ty;
                        let ty_str = quote! { #ty }.to_string().replace(" ", "");
                        if is_program_zero_copy {
                            if ty_str.starts_with("ZcVec<") {
                                // ZcVec<T> → Span<'info, T>: explicit zero-copy opt-in
                                let inner_ty_str =
                                    ty_str.trim_start_matches("ZcVec<").trim_end_matches('>');
                                let new_ty: syn::Type = syn::parse_str(&format!(
                                    "naclac_lang::prelude::Span<'info, {}>",
                                    inner_ty_str
                                ))
                                .unwrap();
                                pat_type.ty = Box::new(new_ty);
                            } else if ty_str.starts_with("ZcString") {
                                // ZcString → ZcString<'info>: explicit zero-copy opt-in
                                let new_ty: syn::Type =
                                    syn::parse_str("naclac_lang::prelude::ZcString<'info>")
                                        .unwrap();
                                pat_type.ty = Box::new(new_ty);
                            }
                            // Standard Vec<T> and String: leave as-is, they use heap allocation
                        }

                        // Strip any references to get to the underlying Context type
                        let mut inner_ty = (*pat_type.ty).clone();

                        if let Type::Reference(r) = &inner_ty {
                            inner_ty = (*r.elem).clone();
                        }

                        if let Type::Path(type_path) = &mut inner_ty {
                            if let Some(segment) = type_path.path.segments.last_mut() {
                                if segment.ident == "Context" {
                                    has_context = true;
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
                                                    let ident = last_seg.ident.clone();
                                                    if !ident.to_string().ends_with("Hydrated") {
                                                        last_seg.ident =
                                                            format_ident!("{}Hydrated", ident);
                                                    }
                                                    // Always ensure <'info> is present on the Hydrated struct
                                                    // to match Context<'info, T>, which is invariant over 'info
                                                    let generic_args: syn::AngleBracketedGenericArguments = syn::parse_quote! { <'info> };
                                                    last_seg.arguments =
                                                        syn::PathArguments::AngleBracketed(
                                                            generic_args,
                                                        );

                                                    // Ensure the wrapper function has the 'info lifetime param
                                                    let has_info = new_func
                                                        .sig
                                                        .generics
                                                        .lifetimes()
                                                        .any(|lt| lt.lifetime.ident == "info");
                                                    if !has_info {
                                                        new_func
                                                            .sig
                                                            .generics
                                                            .params
                                                            .push(syn::parse_quote!('info));
                                                    }
                                                }
                                            }

                                            // Ensure Context has precisely <'info, T<'info>> or <'a, 'info, T<'info>>
                                            let mut has_explicit_info = false;
                                            let mut new_args = syn::punctuated::Punctuated::<
                                                syn::GenericArgument,
                                                syn::Token![,],
                                            >::new(
                                            );
                                            for arg in &args.args {
                                                if let syn::GenericArgument::Lifetime(lt) = arg {
                                                    if lt.ident == "info" {
                                                        has_explicit_info = true;
                                                    }
                                                    new_args.push(arg.clone());
                                                }
                                            }
                                            if !has_explicit_info {
                                                new_args.push(syn::parse_quote!('info));
                                            }
                                            // Re-add the type arg (which we updated in-place)
                                            new_args.push(args.args[idx].clone());
                                            args.args = new_args;
                                        }
                                    }

                                    // Standardize the parameter type to Context<'info, T<'info>>
                                    // avoiding &mut references to prevent type mismatches in CPI wrappers.
                                    pat_type.ty = Box::new(syn::parse_quote! { #inner_ty });
                                }
                            }
                        }
                    }
                }

                if has_context {
                    let has_info_lifetime = new_func.sig.generics.params.iter().any(|p| {
                        if let syn::GenericParam::Lifetime(lt) = p {
                            lt.lifetime.ident == "info"
                        } else {
                            false
                        }
                    });
                    if !has_info_lifetime {
                        new_func
                            .sig
                            .generics
                            .params
                            .insert(0, syn::parse_quote! { 'info });
                    }
                }

                // --- DISCRIMINATOR HASH ---
                let func_str = func_name.to_string();
                let preimage = format!("global:{}", func_str);
                let mut hasher = Sha256::new();
                hasher.update(preimage.as_bytes());
                let disc: [u8; 8] = hasher.finalize()[..8].try_into().unwrap();
                let b0 = disc[0];
                let b1 = disc[1];
                let b2 = disc[2];
                let b3 = disc[3];
                let b4 = disc[4];
                let b5 = disc[5];
                let b6 = disc[6];
                let b7 = disc[7];

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

                // --- DETECT ZERO-COPY ATTRIBUTE ---
                let is_zero_copy = is_program_zero_copy
                    || func.attrs.iter().any(|attr| {
                        let attr_str = quote::quote! { #attr }.to_string();
                        attr_str.contains("instruction") && attr_str.contains("zero_copy")
                    });

                // --- ARGUMENT DESERIALIZATION LOGIC ---
                let mut deserialization_logic = Vec::new();
                let mut arg_tuple_names = Vec::new();
                let mut arg_tuple_types = Vec::new();
                let mut slice_arg_names = Vec::new();

                deserialization_logic.push(quote! { let mut __ix_offset: usize = 8; });

                for (name, ty) in arg_names.iter().zip(arg_types.iter()) {
                    let ty_str = quote! { #ty }.to_string();

                    if ty_str.contains("& [u8]") || ty_str.contains("&[u8]") {
                        slice_arg_names.push(*name);
                        continue;
                    }

                    arg_tuple_names.push(*name);
                    arg_tuple_types.push(*ty);

                    let is_zero_copy_vec = ty_str.contains("ZcVec") || ty_str.contains("Span");
                    let is_zero_copy_string = ty_str.contains("ZcString");
                    let is_zero_copy_collection = is_zero_copy_vec || is_zero_copy_string;
                    let is_heap_vec = ty_str.contains("Vec") && !is_zero_copy_vec;
                    let is_heap_string = ty_str.contains("String") && !is_zero_copy_string;
                    let is_heap_collection = is_heap_vec || is_heap_string;

                    if is_zero_copy && !is_zero_copy_collection && !is_heap_collection {
                        deserialization_logic.push(quote! {
                            let #name = {
                                let __sz = <#ty as naclac_lang::prelude::NaclacPod>::naclac_size();
                                if instruction_data.len() < __ix_offset + __sz {
                                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                }
                                let __val = <#ty as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                                    &instruction_data[__ix_offset..__ix_offset + __sz]
                                );
                                __ix_offset += __sz;
                                __val
                            };
                        });
                    } else if is_zero_copy && (is_zero_copy_collection || is_heap_collection) {
                        let inner_ty = if let Type::Path(tp) = &***ty {
                            if let Some(seg) = tp.path.segments.last() {
                                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                    if let Some(syn::GenericArgument::Type(inner)) =
                                        args.args.first()
                                    {
                                        Some(inner)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let __v_logic = if is_zero_copy_collection {
                            if is_zero_copy_vec {
                                if let Some(inner) = &inner_ty {
                                    quote! {
                                        let __v = <naclac_lang::prelude::Span<'_, #inner>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                        __ix_offset += __len;
                                        __v
                                    }
                                } else {
                                    quote! {
                                        let __v = <naclac_lang::prelude::Span<'_, u8>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                        __ix_offset += __len;
                                        __v
                                    }
                                }
                            } else {
                                quote! {
                                    let __v = <naclac_lang::prelude::ZcString<'_>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])?;
                                    __ix_offset += __len;
                                    __v
                                }
                            }
                        } else {
                            let is_nested_vec = is_heap_vec
                                && (ty_str.contains("Vec < Vec") || ty_str.contains("Vec<Vec"));
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
                                    let mut __outer_vec = naclac_lang::prelude::Vec::with_capacity(__len as usize);
                                    for _ in 0..__len {
                                        if instruction_data.len() < __ix_offset + 4 {
                                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                        }
                                        let mut __inner_len_bytes = [0u8; 4];
                                        __inner_len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                                        let __inner_len = u32::from_le_bytes(__inner_len_bytes) as usize;
                                        __ix_offset += 4;
                                        if instruction_data.len() < __ix_offset + __inner_len {
                                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                        }
                                        let mut __inner_vec = naclac_lang::prelude::Vec::with_capacity(__inner_len);
                                        __inner_vec.extend_from_slice(&instruction_data[__ix_offset..__ix_offset+__inner_len]);
                                        __outer_vec.push(__inner_vec);
                                        __ix_offset += __inner_len;
                                    }
                                    let __v = __outer_vec;
                                    __v
                                }
                            } else {
                                if let Some(inner) = &inner_ty {
                                    if quote! { #inner }.to_string() == "u8" {
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
                                            let mut __v = naclac_lang::prelude::Vec::with_capacity(__len as usize);
                                            let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                                            for _ in 0..__len {
                                                if instruction_data.len() < __ix_offset + __sz {
                                                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                                }
                                                let __item = <#inner as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                                                    &instruction_data[__ix_offset..__ix_offset + __sz]
                                                );
                                                __v.push(__item);
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

                        deserialization_logic.push(quote! {
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
                        });
                    } else {
                        // Seamless Dual-Backend Routing:
                        // Emits `cfg`-gated deserialization logic for both Pinocchio (NaclacPod)
                        // and standard Solana (Borsh).
                        deserialization_logic.push(quote! {
                            #[cfg(feature = "pinocchio")]
                            let #name = {
                                let __sz = <#ty as naclac_lang::prelude::NaclacPod>::naclac_size();
                                if instruction_data.len() < __ix_offset + __sz {
                                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                                }
                                let __val = <#ty as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                                    &instruction_data[__ix_offset..__ix_offset + __sz]
                                );
                                __ix_offset += __sz;
                                __val
                            };
                            #[cfg(not(feature = "pinocchio"))]
                            let #name = {
                                let mut __r: &[u8] = &instruction_data[__ix_offset..];
                                let __val = <#ty as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                                __ix_offset = instruction_data.len() - __r.len();
                                __val
                            };
                        });
                    }
                }

                let _args_deserialization = quote! {
                    #( #deserialization_logic )*
                };

                // Args are now local variables — reference directly (no __args.name)
                let arg_injections = arg_names.iter().map(|name| {
                    if slice_arg_names.contains(name) {
                        quote! { , &instruction_data[8..] }
                    } else {
                        quote! { , #name }
                    }
                });
                let arg_injections = quote! { #(#arg_injections)* };

                // --- THE DISPATCH FUNCTION ---
                let dispatch_func_name = format_ident!("__dispatch_{}", func_name);
                processed_content_items.push(syn::parse_quote! {
                    pub(crate) fn #dispatch_func_name<'info>(
                        program_id: &'info naclac_lang::prelude::Pubkey,
                        accounts: &'info [naclac_lang::prelude::AccountInfo<'info>],
                        instruction_data: &'info [u8],
                    ) -> naclac_lang::prelude::Result {

                        // Deserialize args for passing to the instruction handler
                        #_args_deserialization

                        // Load & Validate Accounts (also deserializes args for constraint checks)
                        let (mut accounts, remaining_accounts, bumps) = #ctx_struct::load_and_validate(
                            program_id,
                            accounts,
                            instruction_data
                        )?;


                        // Hydrate (Open Zero-Copy Guards)
                        let mut hydrated_accounts = accounts.hydrate(bumps)?;

                        // Execute using Safe Pointer Abstraction
                        // SAFETY: We create a mutable pointer to the `hydrated_accounts` struct.
                        // We immediately dereference it to create `accounts_ref`. This is sound because
                        // `hydrated_accounts` is pinned to the current stack frame and will not be moved
                        // or dropped until the instruction execution completes and `teardown` is called.
                        let accounts_ptr = &mut hydrated_accounts as *mut _;
                        let accounts_ref = unsafe { &mut *accounts_ptr };

                        let ctx = naclac_lang::prelude::Context::new(
                            program_id,
                            accounts_ref,
                            remaining_accounts,
                            bumps,
                        );

                        let __result = #mod_name::#func_name(ctx #arg_injections);

                        // Execute Account Teardown:
                        // Persists any mutations made to the hydrated_accounts back into the
                        // on-chain account data buffers.
                        if __result.is_ok() {
                            hydrated_accounts.teardown(program_id)?;
                        }

                        __result
                    }
                });

                // --- THE MATCH ARM ---
                match_arms.push(quote! {
                    [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7] => {
                        #mod_name::#dispatch_func_name(program_id, accounts, instruction_data)
                    }
                });
            }
            processed_content_items.push(Item::Fn(new_func));
        } else {
            processed_content_items.push(item.clone());
        }
    }

    // --- IDL PROCESSORS ---
    let idl_create_disc = Sha256::digest(b"naclac:idl_create");
    let ic_b0 = idl_create_disc[0];
    let ic_b1 = idl_create_disc[1];
    let ic_b2 = idl_create_disc[2];
    let ic_b3 = idl_create_disc[3];
    let ic_b4 = idl_create_disc[4];
    let ic_b5 = idl_create_disc[5];
    let ic_b6 = idl_create_disc[6];
    let ic_b7 = idl_create_disc[7];

    let idl_write_disc = Sha256::digest(b"naclac:idl_write");
    let iw_b0 = idl_write_disc[0];
    let iw_b1 = idl_write_disc[1];
    let iw_b2 = idl_write_disc[2];
    let iw_b3 = idl_write_disc[3];
    let iw_b4 = idl_write_disc[4];
    let iw_b5 = idl_write_disc[5];
    let iw_b6 = idl_write_disc[6];
    let iw_b7 = idl_write_disc[7];

    let idl_resize_disc = Sha256::digest(b"naclac:idl_resize");
    let ir_b0 = idl_resize_disc[0];
    let ir_b1 = idl_resize_disc[1];
    let ir_b2 = idl_resize_disc[2];
    let ir_b3 = idl_resize_disc[3];
    let ir_b4 = idl_resize_disc[4];
    let ir_b5 = idl_resize_disc[5];
    let ir_b6 = idl_resize_disc[6];
    let ir_b7 = idl_resize_disc[7];

    match_arms.push(quote! {
        #[cfg(all(feature = "idl-build", not(feature = "pinocchio")))]
        [#ic_b0, #ic_b1, #ic_b2, #ic_b3, #ic_b4, #ic_b5, #ic_b6, #ic_b7] => {
            __naclac_idl::__idl_create(program_id, accounts, &instruction_data[8..])
        }
    });

    match_arms.push(quote! {
        #[cfg(all(feature = "idl-build", not(feature = "pinocchio")))]
        [#iw_b0, #iw_b1, #iw_b2, #iw_b3, #iw_b4, #iw_b5, #iw_b6, #iw_b7] => {
            __naclac_idl::__idl_write(program_id, accounts, &instruction_data[8..])
        }
    });

    match_arms.push(quote! {
        #[cfg(all(feature = "idl-build", not(feature = "pinocchio")))]
        [#ir_b0, #ir_b1, #ir_b2, #ir_b3, #ir_b4, #ir_b5, #ir_b6, #ir_b7] => {
            __naclac_idl::__idl_resize(program_id, accounts, &instruction_data[8..])
        }
    });

    // --- AUTOMATIC CPI GENERATION ---
    let mut cpi_instructions = extract_automatic_instructions(&content_items);
    if is_program_zero_copy {
        for ix in &mut cpi_instructions {
            ix.is_zero_copy = true;
        }
    }
    let cpi_module = generate_cpi_module(&cpi_instructions, &quote!(naclac_lang));

    // --- FINAL MACRO EXPANSION ---
    let expanded = quote! {
        #cpi_module

        #[cfg(all(not(feature = "no-entrypoint"), not(feature = "pinocchio")))]
        naclac_lang::solana_program::entrypoint!(process_instruction);

        #[cfg(all(not(feature = "no-entrypoint"), feature = "pinocchio"))]
        naclac_lang::pinocchio::entrypoint!(process_instruction);

        // Unified Entrypoint — standard signature required by solana/pinocchio entrypoint! macro
        #[cfg(not(feature = "pinocchio"))]
        pub fn process_instruction(
            program_id: &naclac_lang::prelude::Pubkey,
            accounts: &[naclac_lang::prelude::AccountInfo<'_>],
            instruction_data: &[u8],
        ) -> naclac_lang::prelude::Result {
            // SAFETY: The Solana runtime guarantees these references are valid for the
            // entire program invocation. We transmute the lifetime to 'static so the
            // inner dispatcher can safely bind them to Context<'info, T> where 'info
            // is invariant across the execution.
            unsafe {
                let program_id: &'static naclac_lang::prelude::Pubkey = core::mem::transmute(program_id);
                let accounts: &'static [naclac_lang::prelude::AccountInfo<'static>] = core::mem::transmute(accounts);
                let instruction_data: &'static [u8] = core::mem::transmute(instruction_data);
                process_instruction_inner(program_id, accounts, instruction_data)
            }
        }

        #[cfg(feature = "pinocchio")]
        pub fn process_instruction(
            program_id: &naclac_lang::prelude::Address,
            accounts: &[naclac_lang::prelude::AccountView],
            instruction_data: &[u8],
        ) -> naclac_lang::prelude::ProgramResult {
            // SAFETY: Convert pinocchio native types (`AccountView`) to framework `AccountInfo` wrappers.
            // Pinocchio guarantees the underlying memory layout is identical (a thin wrapper around
            // the runtime pointer). The runtime guarantees these pointers are valid for the entire invocation.
            unsafe {
                let accounts_info: &'static [naclac_lang::prelude::AccountInfo<'static>] = core::mem::transmute(
                    core::slice::from_raw_parts(
                        accounts.as_ptr() as *const naclac_lang::prelude::AccountInfo<'static>,
                        accounts.len(),
                    )
                );
                let program_id: &'static naclac_lang::prelude::Pubkey = core::mem::transmute(program_id);
                let instruction_data: &'static [u8] = core::mem::transmute(instruction_data);
                process_instruction_inner(program_id, accounts_info, instruction_data)
            }
        }

        #[inline(always)]
        fn process_instruction_inner(
            program_id: &'static naclac_lang::prelude::Pubkey,
            accounts: &'static [naclac_lang::prelude::AccountInfo<'static>],
            instruction_data: &'static [u8],
        ) -> naclac_lang::prelude::Result {
            if instruction_data.len() < 8 {
                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
            }
            let mut instruction_discriminator = [0u8; 8];
            instruction_discriminator.copy_from_slice(&instruction_data[0..8]);
            match instruction_discriminator {
                #(#match_arms),*
                _ => Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
            }
        }



        // Re-inject the developer's original module logic.
        #mod_vis mod #mod_name {
            use super::*;
            #(#processed_content_items)*
        }

        // Inject the IDL processor module for on-chain IDL management.
        #[cfg(all(feature = "idl-build", not(feature = "pinocchio")))]
        pub mod __naclac_idl {
            use naclac_lang::solana_program::{
                account_info::{next_account_info, AccountInfo},
                program::invoke_signed,
                pubkey::Pubkey,
            };
            use naclac_lang::prelude::Result;
            use naclac_lang::solana_system_interface::instruction as system_instruction;

            pub fn __idl_create(
                program_id: &Pubkey,
                accounts: &[AccountInfo],
                instruction_data: &[u8],
            ) -> Result {
                let account_info_iter = &mut accounts.iter();
                let payer = next_account_info(account_info_iter)?;
                let idl_pda = next_account_info(account_info_iter)?;
                let system_program_info = next_account_info(account_info_iter)?;

                if !payer.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }

                let seed: &[u8] = b"anchor:idl";
                let (expected_pda, bump) = Pubkey::find_program_address(&[seed, program_id.as_ref()], program_id);
                if idl_pda.key != &expected_pda {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }

                if instruction_data.len() < 16 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }

                let mut lamports_b = [0u8; 8];
                let mut space_b = [0u8; 8];
                lamports_b.copy_from_slice(&instruction_data[0..8]);
                space_b.copy_from_slice(&instruction_data[8..16]);

                let lamports = u64::from_le_bytes(lamports_b);
                let space = u64::from_le_bytes(space_b);

                let create_ix = system_instruction::create_account(
                    payer.key,
                    idl_pda.key,
                    lamports,
                    space,
                    program_id,
                );

                invoke_signed(
                    &create_ix,
                    &[payer.clone(), idl_pda.clone(), system_program_info.clone()],
                    &[&[seed, program_id.as_ref(), &[bump]]],
                )?;

                Ok(())
            }

            pub fn __idl_write(
                program_id: &Pubkey,
                accounts: &[AccountInfo],
                instruction_data: &[u8],
            ) -> Result {
                let account_info_iter = &mut accounts.iter();
                let signer = next_account_info(account_info_iter)?;
                let idl_pda = next_account_info(account_info_iter)?;

                if !signer.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }

                if idl_pda.owner != program_id {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(0));
                }

                let data = idl_pda.try_borrow_data()?;
                if data.len() >= 40 {
                    let mut auth_bytes = [0u8; 32];
                    auth_bytes.copy_from_slice(&data[8..40]);
                    let auth = Pubkey::new_from_array(auth_bytes);
                    if auth != Pubkey::default() && auth != *signer.key {
                        return Err(naclac_lang::prelude::NaclacError::Unauthorized.err(0));
                    }
                }
                drop(data);

                if instruction_data.len() < 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }

                let mut offset_b = [0u8; 4];
                offset_b.copy_from_slice(&instruction_data[0..4]);
                let offset = u32::from_le_bytes(offset_b) as usize;

                let payload = &instruction_data[4..];

                let mut mut_data = idl_pda.try_borrow_mut_data()?;
                if offset + payload.len() > mut_data.len() {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }

                mut_data[offset..offset + payload.len()].copy_from_slice(payload);
                Ok(())
            }

            pub fn __idl_resize(
                program_id: &Pubkey,
                accounts: &[AccountInfo],
                instruction_data: &[u8],
            ) -> Result {
                let account_info_iter = &mut accounts.iter();
                let authority = next_account_info(account_info_iter)?;
                let idl_pda = next_account_info(account_info_iter)?;
                let _system_program = next_account_info(account_info_iter)?;

                if !authority.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }

                if idl_pda.owner != program_id {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(0));
                }

                // Verify authority matches existing IDL metadata
                {
                    let data = idl_pda.try_borrow_data()?;
                    if data.len() >= 40 {
                        let mut auth_bytes = [0u8; 32];
                        auth_bytes.copy_from_slice(&data[8..40]);
                        let stored_auth = Pubkey::new_from_array(auth_bytes);
                        if stored_auth != Pubkey::default() && stored_auth != *authority.key {
                            return Err(naclac_lang::prelude::NaclacError::Unauthorized.err(0));
                        }
                    }
                }

                if instruction_data.len() < 8 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }

                let mut new_size_b = [0u8; 8];
                new_size_b.copy_from_slice(&instruction_data[0..8]);
                let new_size = u64::from_le_bytes(new_size_b) as usize;

                // 1. Perform Realloc
                idl_pda.resize(new_size)?;

                // 2. Adjust Lamports for Rent Exemption
                let rent = naclac_lang::solana_program::rent::Rent::default();
                let new_minimum_balance = rent.minimum_balance(new_size);

                let current_lamports = idl_pda.lamports();

                if new_minimum_balance > current_lamports {
                    // Need more lamports
                    let diff = new_minimum_balance.saturating_sub(current_lamports);
                    let system_program_info = next_account_info(account_info_iter)?;
                    let create_ix = naclac_lang::solana_system_interface::instruction::transfer(
                        authority.key,
                        idl_pda.key,
                        diff,
                    );
                    naclac_lang::solana_program::program::invoke(
                        &create_ix,
                        &[authority.clone(), idl_pda.clone(), system_program_info.clone()],
                    )?;
                } else if new_minimum_balance < current_lamports {
                    // Reclaim excess lamports
                    let diff = current_lamports.saturating_sub(new_minimum_balance);
                    **idl_pda.lamports.borrow_mut() -= diff;
                    **authority.lamports.borrow_mut() += diff;
                }

                Ok(())
            }
        }
    };

    expanded.into()
}
