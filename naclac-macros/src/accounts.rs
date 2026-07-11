//! # Accounts Macro Logic
//!
//! Handles the expansion of the `#[derive(Accounts)]` macro, generating highly
//! optimized account validation, instruction deserialization, and Zero-Copy safety logic.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::instruction::close_account;
use crate::instruction::init_cpi;
use crate::instruction::parser;
use crate::instruction::realloc;
use crate::instruction::security;

/// Entrypoint for `#[derive(Accounts)]` expansion.
///
/// Processes field constraints, performs security checks, generates instruction data
/// deserialization, and manages the hydration/teardown pipeline. Zero-copy vs Borsh
/// codegen is chosen automatically (see `is_zero_copy` below) — there used to be a
/// separate `#[derive(AccountsLoader)]` to force zero-copy mode, but it did nothing
/// besides that, and is now fully subsumed by the automatic detection.
pub fn expand_derive_accounts(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;

    // 1. Ensure the macro is only used on a Struct with named fields
    let fields_named = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named,
            _ => panic!("Naclac: #[derive(Accounts)] must have named fields"),
        },
        _ => panic!("Naclac: #[derive(Accounts)] can only be used on structs"),
    };

    // Validate that no fields use the Pubkey type
    for field in &fields_named.named {
        let ty = &field.ty;
        let ty_str = quote! { #ty }.to_string().replace(" ", "");
        if ty_str.contains("Pubkey") {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    // 2. Parse the fields using our new V2 parser
    let parsed_fields = match parser::parse_struct_fields(fields_named) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };
    let mut instr_names: Vec<syn::Ident> = Vec::new();
    let mut instr_types: Vec<Box<syn::Type>> = Vec::new();
    // Same purpose-built opt-out as program.rs's function-arg handling: an
    // argument written as `#[allow_heap] name: Vec<T>` skips the heap-collection
    // warning. No stripping needed here (unlike program.rs) — this lives inside
    // `#[instruction(...)]`, a registered derive helper attribute whose contents
    // rustc never independently parses/validates.
    let mut heap_allowed_names: std::collections::HashSet<syn::Ident> =
        std::collections::HashSet::new();

    for attr in &ast.attrs {
        if attr.path().is_ident("instruction") {
            if let Ok(parsed_args) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::FnArg, syn::Token![,]>::parse_terminated,
            ) {
                for arg in parsed_args {
                    if let syn::FnArg::Typed(pat_type) = arg {
                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                            if pat_type
                                .attrs
                                .iter()
                                .any(|a| a.path().is_ident("allow_heap"))
                            {
                                heap_allowed_names.insert(pat_ident.ident.clone());
                            }
                            instr_names.push(pat_ident.ident.clone());
                            instr_types.push(pat_type.ty.clone());
                        }
                    }
                }
            }
        }
    }

    // Validate that no instruction arguments use the Pubkey type
    for ty in &instr_types {
        let ty_str = quote! { #ty }.to_string().replace(" ", "");
        if ty_str.contains("Pubkey") {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    let is_zero_copy =
        crate::caller_has_feature("pinocchio") || !crate::caller_has_feature("borsh");

    let ix_deserializer = if !instr_names.is_empty() {
        let mut pod_instr_names: Vec<&syn::Ident> = Vec::new();
        let mut pod_instr_types: Vec<&Box<syn::Type>> = Vec::new();
        let mut dynamic_instr_names: Vec<&syn::Ident> = Vec::new();
        let mut dynamic_instr_types: Vec<&Box<syn::Type>> = Vec::new();
        let mut slice_instr_names: Vec<&syn::Ident> = Vec::new();

        for (name, ty) in instr_names.iter().zip(instr_types.iter()) {
            let ty_str = quote! { #ty }.to_string().replace(" ", "");
            if ty_str.contains("&[u8]") {
                slice_instr_names.push(name);
            } else if ty_str.contains("Vec") || ty_str.contains("String") {
                dynamic_instr_names.push(name);
                dynamic_instr_types.push(ty);
            } else {
                pod_instr_names.push(name);
                pod_instr_types.push(ty);
            }
        }

        if is_zero_copy {
            let pod_reads = pod_instr_names.iter().zip(pod_instr_types.iter()).map(|(name, ty)| {
                quote! {
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
                }
            });

            // Classification mirrors program.rs's #[program]-function-arg handling:
            // `ZcVec<T>`/`Span<T>`/`ZcString` are explicit zero-copy opt-ins parsed
            // via Span/ZcString (no allocation); a bare `Vec<T>`/`String` heap-allocates
            // (valid, e.g. for interop with an existing Vec/String-shaped API) and gets
            // a silenceable warning nudging toward the zero-copy types instead. Kept
            // consistent with program.rs deliberately, so the same source syntax can't
            // mean two different things depending on which macro processes it.
            let dynamic_reads = dynamic_instr_names.iter().zip(dynamic_instr_types.iter()).map(|(name, ty)| {
                let ty_str = quote! { #ty }.to_string().replace(" ", "");
                let is_zc_vec = ty_str.contains("ZcVec") || ty_str.contains("Span");
                let is_zc_string = ty_str.contains("ZcString");
                let is_heap_vec = ty_str.contains("Vec") && !is_zc_vec;
                let is_heap_string = ty_str.contains("String") && !is_zc_string;

                let inner_ty = if let syn::Type::Path(tp) = &***ty {
                    tp.path.segments.last().and_then(|seg| {
                        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                            args.args.iter().find_map(|a| match a {
                                syn::GenericArgument::Type(t) => Some(t),
                                _ => None,
                            })
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };

                let __v_logic = if is_zc_vec {
                    if let Some(inner) = inner_ty {
                        quote! { <naclac_lang::prelude::Span<#inner>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                    } else {
                        quote! { <naclac_lang::prelude::Span<u8>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                    }
                } else if is_zc_string {
                    quote! { <naclac_lang::prelude::ZcString>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                } else if is_heap_string {
                    quote! {
                        naclac_lang::prelude::String::from_utf8(instruction_data[__ix_offset..__ix_offset+__len].to_vec())
                            .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?
                    }
                } else if let Some(inner) = inner_ty {
                    let inner_str = quote! { #inner }.to_string().replace(" ", "");
                    if inner_str == "u8" {
                        quote! { instruction_data[__ix_offset..__ix_offset+__len].to_vec() }
                    } else {
                        quote! {
                            {
                                let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                                let mut __v = naclac_lang::prelude::Vec::with_capacity(__len / __sz.max(1));
                                let mut __p = __ix_offset;
                                while __p < __ix_offset + __len {
                                    __v.push(<#inner as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                                        &instruction_data[__p..__p + __sz]
                                    ));
                                    __p += __sz;
                                }
                                __v
                            }
                        }
                    }
                } else {
                    quote! { return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)); }
                };

                let warning_tokens = if (is_heap_vec || is_heap_string) && !heap_allowed_names.contains(*name) {
                    let found_ty = if is_heap_vec { "Vec<T>" } else { "String" };
                    let suggested_ty = if is_heap_vec { "ZcVec<T>" } else { "ZcString" };
                    crate::heap_collection_warning(&name.to_string(), found_ty, suggested_ty)
                } else {
                    quote! {}
                };

                quote! {
                    #warning_tokens
                    let #name = {
                        if instruction_data.len() < __ix_offset + 4 {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let mut __len_bytes = [0u8; 4];
                        __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                        let __len = u32::from_le_bytes(__len_bytes) as usize;
                        __ix_offset += 4;
                        if instruction_data.len() < __ix_offset + __len {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let __v = #__v_logic;
                        __ix_offset += __len;
                        __v
                    };
                }
            });

            let slice_reads = slice_instr_names.iter().map(|name| {
                quote! { let #name = &instruction_data[__ix_offset..]; }
            });

            quote! {
                let mut __ix_offset: usize = 8;
                #(#pod_reads)*
                #(#dynamic_reads)*
                #(#slice_reads)*
            }
        } else {
            // Standard Borsh Path: Heap-allocates instruction arguments dynamically.
            quote! {
                let mut __r: &[u8] = &instruction_data[8..];
                #(
                    let #instr_names = <#instr_types as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                )*
            }
        }
    } else {
        quote! {}
    };

    let mut field_loaders = Vec::new();
    let mut struct_instantiation = Vec::new();

    for field in &parsed_fields {
        let field_name = &field.ident;
        let type_str = &field.type_str;
        let ty = &field.ty;

        if is_zero_copy && (type_str.contains("Vec") || type_str.contains("String")) {
            return syn::Error::new_spanned(
                ty,
                format!(
                    "Naclac Error: In Zero-Copy mode, heap-based types like 'Vec' and 'String' are not valid Accounts-struct field wrapper types for '{}'. Account fields must be an account wrapper type (Account<T>, AccountLoader<T>, Signer, Program<T>, etc.).",
                    field_name
                ),
            )
            .to_compile_error()
            .into();
        }

        let idx = field.index;
        let (metadata_checks, constraint_checks) =
            security::generate_security_checks(field, &parsed_fields, is_zero_copy);
        let init_logic = init_cpi::generate_init_cpi(field, &parsed_fields, is_zero_copy);

        let loader_fn = if field.is_mut && field.init_config.is_none() && !field.is_init_if_needed {
            quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from_mut }
        } else {
            quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from }
        };
        let loader_logic = quote! {
            let info = &__views[#idx];
            #metadata_checks
            #init_logic
            let mut #field_name = #loader_fn(info, #idx)?;
            #constraint_checks
        };

        field_loaders.push(loader_logic);
        struct_instantiation.push(quote! { #field_name });
    }

    // --- Real-size stack-frame safety checks (real size_of, not a string guess) ---
    // Anything other than a thin marker (Signer/Program/plain AccountInfo) is
    // "data-carrying" and gets a per-field assert using the field's REAL,
    // resolved size. Already-boxed fields (`Box<Account<T>>`) trivially pass —
    // size_of::<Box<_>>() is just a pointer (8 bytes) — so there is no need to
    // special-case "skip fields that are already boxed": boxing a field just
    // makes its own real size tiny, which naturally satisfies both checks below.
    let size_guard_consts = if is_zero_copy {
        quote! {}
    } else {
        let data_carrying_fields: Vec<(&syn::Ident, &syn::Type)> = parsed_fields
            .iter()
            .filter(|f| f.type_str.contains("Account") && !f.type_str.contains("AccountInfo"))
            .map(|f| (&f.ident, &f.ty))
            .collect();

        let per_field_asserts = data_carrying_fields.iter().map(|(ident, ty)| {
            let msg = format!(
                "naclac: field `{}` is large for a single stack frame — wrap it as `Box<{}>` to keep `load_and_validate` under the SBF 4KB stack limit",
                ident,
                quote! { #ty }.to_string().replace(" ", "")
            );
            quote! {
                const _: () = assert!(core::mem::size_of::<#ty>() <= 300, #msg);
            }
        });

        let aggregate_assert = if data_carrying_fields.is_empty() {
            quote! {}
        } else {
            let field_list = data_carrying_fields
                .iter()
                .map(|(ident, _)| ident.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let sizes = data_carrying_fields
                .iter()
                .map(|(_, ty)| quote! { core::mem::size_of::<#ty>() });
            let msg = format!(
                "naclac: this Accounts struct's data-carrying fields ({}) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
                field_list
            );
            quote! {
                const _: () = assert!(0usize #( + #sizes )* <= 1700, #msg);
            }
        };

        quote! {
            #(#per_field_asserts)*
            #aggregate_assert
        }
    };

    // --- Security Check Generation ---
    let relational_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let relational_checks_me =
        security::generate_relational_checks(&parsed_fields, quote! { __me });
    let realloc_logic = realloc::generate_realloc_logic(&parsed_fields);
    let close_logic = close_account::generate_close_logic(&parsed_fields);

    let save_logic: Vec<proc_macro2::TokenStream> = if is_zero_copy {
        Vec::new()
    } else {
        let mut v = Vec::new();
        for field in &parsed_fields {
            let field_name = &field.ident;
            if field.is_mut || field.type_str.contains("mut") {
                v.push(quote! {
                    naclac_lang::prelude::NaclacAccount::exit(&self.#field_name, program_id)?;
                });
            }
        }
        v
    };

    let mut bumps_fields = Vec::new();
    let mut bumps_instantiations = Vec::new();
    for field in &parsed_fields {
        if field.pda_seed.is_some() {
            let field_name = &field.ident;
            let bump_var = quote::format_ident!("__bump_{}", field_name);
            bumps_fields.push(quote! { pub #field_name: u8 });
            bumps_instantiations.push(quote! { #field_name: #bump_var });
        }
    }

    let bumps_struct_name = quote::format_ident!("{}Bumps", struct_name);
    let bumps_struct_def = quote! {
        #[derive(Clone, Copy, Default)]
        pub struct #bumps_struct_name {
            #(#bumps_fields),*
        }
    };

    let bumps_instantiation_logic = quote! { #bumps_struct_name { #(#bumps_instantiations),* } };

    // --- Hydration System ---
    let mut mut_zero_copy_fields = Vec::new();

    for field in &parsed_fields {
        let name = &field.ident;
        let type_str = &field.type_str;

        if type_str.contains("AccountLoader")
            || type_str.contains("ZcAccount")
            || (is_zero_copy && type_str.contains("Account") && !type_str.contains("AccountInfo"))
        {
            let is_token_account = type_str.contains("TokenAccount");
            if (type_str.contains("mut") || field.is_mut)
                && field.pda_seed.is_some()
                && field.pda_bump.as_deref() == Some("__naclac_auto_bump")
                && !is_token_account
            {
                mut_zero_copy_fields.push(name);
            }
        }
    }

    let hydrated_struct_def = quote! {};
    let hydration_impl = quote! {};

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let n_fields = parsed_fields.len();
    let field_names: Vec<_> = parsed_fields.iter().map(|f| &f.ident).collect();
    let is_signers: Vec<_> = parsed_fields
        .iter()
        .map(|f| f.type_str.contains("Signer"))
        .collect();
    let is_writables: Vec<_> = parsed_fields
        .iter()
        .map(|f| f.is_mut || f.type_str.contains("mut"))
        .collect();
    let is_aliases: Vec<_> = parsed_fields.iter().map(|f| f.is_alias).collect();

    let n_declared_accounts = parsed_fields.len();

    let mut_mask_steps: Vec<_> = parsed_fields
        .iter()
        .enumerate()
        .filter_map(|(i, field)| {
            let is_mut = field.is_mut || field.type_str.contains("mut");
            if is_mut && !field.is_alias {
                Some(quote! {
                    __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, #i);
                })
            } else {
                None
            }
        })
        .collect();

    let mut_mask_body = if mut_mask_steps.is_empty() {
        quote! { [0u64; 4] }
    } else {
        quote! {{
            let mut __mask = [0u64; 4];
            #(#mut_mask_steps)*
            __mask
        }}
    };

    let payload_type = quote! { Self };
    let as_mut_payload_body = quote! { payload };
    let inst_me = quote! { let mut __me = Self { #(#struct_instantiation),* }; };

    let loadable_accounts_impl = quote! {
        #[cfg(not(feature = "pinocchio"))]
        impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for #struct_name #ty_generics #where_clause {
            type Payload = #payload_type;

            fn load_and_validate(
                program_id: &naclac_lang::prelude::Address,
                accounts: &'a [naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
                __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
            ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, #bumps_struct_name> {
                // Single upfront bounds check.
                if accounts.len() < #n_declared_accounts {
                    return Err(naclac_lang::prelude::ProgramError::NotEnoughAccountKeys);
                }
                if let Some(__dups) = __duplicates {
                    if __dups.intersects(&Self::MUT_MASK) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(0));
                    }
                }
                let __views = accounts;
                #ix_deserializer
                #(#field_loaders)*
                let remaining_accounts = &accounts[#n_declared_accounts..];
                #inst_me
                // Remaining accounts duplicate check
                {
                    let mut __rem_check_idx = #n_declared_accounts;
                    for __rem_acc in remaining_accounts {
                        #(
                            if #is_writables && !#is_aliases {
                                let __decl_addr = naclac_lang::prelude::ToAddress::address(&__me.#field_names);
                                let __rem_addr = naclac_lang::prelude::ToAddress::address(__rem_acc);
                                if __decl_addr == __rem_addr {
                                    return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(__rem_check_idx));
                                }
                            }
                        )*
                        __rem_check_idx += 1;
                    }
                }
                #(#relational_checks_me)*
                Ok((__me, remaining_accounts, #bumps_instantiation_logic))
            }

            #[inline(always)]
            fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                #as_mut_payload_body
            }

            #[inline(always)]
            fn teardown_payload(payload: &mut Self::Payload, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> Result<(), naclac_lang::prelude::ProgramError> {
                payload.teardown(program_id, bumps)
            }
        }
    };

    let load_and_validate_def = quote! {
        /// Loads and validates the accounts provided to the instruction.
        ///
        /// ## Pinocchio path (pre-walked slice)
        /// Takes a pre-walked `&[AccountType]` slice provided by the entrypoint.
        /// Accounts were walked once globally before dispatch — O(1) slice indexing,
        /// no cursor creation, no lookup array allocation per instruction.
        ///
        /// ## Standard path (Borsh/solana-program)
        /// Takes the pre-built `&[AccountType]` slice (unchanged).
        #[cfg(feature = "pinocchio")]
        pub fn load_and_validate(
            program_id: &naclac_lang::prelude::Address,
            __views: &[naclac_lang::prelude::AccountType],
            instruction_data: &[u8],
            __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
        ) -> Result<(Self, &'static [naclac_lang::prelude::AccountType], #bumps_struct_name), naclac_lang::prelude::ProgramError> {
            // Single upfront bounds check — replaces per-account iterator.
            if __views.len() < #n_declared_accounts {
                return Err(naclac_lang::prelude::ProgramError::NotEnoughAccountKeys);
            }
            if let Some(__dups) = __duplicates {
                if __dups.intersects(&Self::MUT_MASK) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(0));
                }
            }
            // Alias for compatibility with init_cpi generated code.
            let accounts = __views;
            #ix_deserializer
            #(#field_loaders)*
            // Remaining accounts are the tail of the pre-walked slice.
            let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                core::mem::transmute(&__views[#n_declared_accounts..])
            };
            let mut __me = Self { #(#struct_instantiation),* };
            // Remaining accounts duplicate check
            {
                let mut __rem_check_idx = #n_declared_accounts;
                for __rem_acc in __remaining {
                    #(
                        if #is_writables && !#is_aliases {
                            let __decl_addr = naclac_lang::prelude::ToAddress::address(&__me.#field_names);
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(__rem_acc);
                            if __decl_addr == __rem_addr {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(__rem_check_idx));
                            }
                        }
                    )*
                    __rem_check_idx += 1;
                }
            }
            #(#relational_checks_me)*
            Ok((__me, __remaining, #bumps_instantiation_logic))
        }

        /// Persists mutations and handles cleanup.
        ///
        /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
        /// so the compiler eliminates the call and the conditional branch in the dispatcher
        /// when the body is empty.
        #[inline(always)]
        pub fn teardown(&mut self, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> naclac_lang::prelude::Result {
            #(#relational_checks)*
            #(#realloc_logic)*
            #( self.#mut_zero_copy_fields.bump = bumps.#mut_zero_copy_fields; )*
            #(#save_logic)*
            #(#close_logic)*
            Ok(())
        }
    };

    let expanded = quote! {
        #bumps_struct_def
        #hydrated_struct_def
        #hydration_impl

        // to_account_metas is an off-chain helper for building transactions from client code.
        // It is dead code inside the on-chain SBF binary, so we gate it to reduce .so size.
        #[cfg(not(target_os = "solana"))]
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_metas(&self) -> [naclac_lang::prelude::AccountMeta; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::AccountMeta {
                            address: (&self.#field_names).address(),
                            is_signer: #is_signers,
                            is_writable: #is_writables,
                        },
                    )*
                ]
            }
        }

        // Unlike to_account_metas above, this isn't gated to off-chain-only:
        // it's usable both from off-chain SDK code and from on-chain instruction
        // bodies that need the full account-info array (e.g. manual CPI construction).
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_infos(&self) -> [naclac_lang::prelude::AccountInfo; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#field_names),
                    )*
                ]
            }
        }

        impl #impl_generics naclac_lang::prelude::Bumps for #struct_name #ty_generics #where_clause {
            type BumpsStruct = #bumps_struct_name;
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub const MUT_MASK: [u64; 4] = #mut_mask_body;
            #load_and_validate_def
        }

        #size_guard_consts

        #loadable_accounts_impl
    };
    expanded.into()
}
