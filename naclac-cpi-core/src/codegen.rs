//! # CPI Code Generation
//!
//! The primary engine for generating type-safe CPI helper functions. It handles
//! dynamic backend routing (Solana vs Pinocchio) and serialization selection
//! (Borsh vs Zero-Copy) based on instruction metadata.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Type};

/// Metadata for a single CPI instruction
pub struct CpiInstruction {
    pub name: Ident,
    pub accounts_struct: syn::Path,
    pub args: Vec<(Ident, Type)>,
    pub discriminator: [u8; 8],
    pub is_zero_copy: bool,
}

/// Generates the full `cpi` module code.
///
/// # Arguments
/// * `instructions` - The list of instructions to generate helpers for.
/// * `naclac_path` - The path to the naclac runtime (e.g., `naclac_lang` or `naclac_interface::runtime`).
pub fn generate_cpi_module(instructions: &[CpiInstruction], naclac_path: &TokenStream) -> TokenStream {
    let helpers: Vec<TokenStream> = instructions
        .iter()
        .map(|ix| generate_helper(ix, naclac_path))
        .collect();

    let account_reexports = instructions.iter().map(|ix| {
        let accounts_struct_path = &ix.accounts_struct;
        let mut cpi_path = accounts_struct_path.clone();
        if let Some(last_seg) = cpi_path.segments.last_mut() {
            let ident = &last_seg.ident;
            use heck::ToSnakeCase;
            last_seg.ident = quote::format_ident!("{}_cpi", ident.to_string().to_snake_case());
        }

        let accounts_struct_ident = &accounts_struct_path.segments.last().unwrap().ident;

        quote! {
            pub use #cpi_path::accounts::#accounts_struct_ident;
        }
    });

    quote! {
        #[cfg(feature = "cpi")]
        pub mod cpi {
            use super::*;

            pub mod accounts {
                use super::*;
                #(#account_reexports)*
            }

            #(#helpers)*
        }
    }
}

fn generate_helper(ix: &CpiInstruction, naclac_path: &TokenStream) -> TokenStream {
    let name = &ix.name;
    let accounts_struct_path = &ix.accounts_struct;
    let accounts_struct_ident = &accounts_struct_path.segments.last().unwrap().ident;
    let arg_names: Vec<_> = ix.args.iter().map(|(n, _)| n.clone()).collect();
    let arg_types: Vec<_> = ix
        .args
        .iter()
        .map(|(_, ty)| {
            let mut t = ty.clone();
            if let syn::Type::Reference(r) = &mut t {
                if r.lifetime.is_none() {
                    r.lifetime = Some(syn::Lifetime::new("'info", proc_macro2::Span::call_site()));
                }
            }
            t
        })
        .collect();

    let disc = &ix.discriminator;
    let disc_array = quote! { [#(#disc),*] };

    // Struct name for instruction data
    use heck::AsPascalCase;
    let ix_data_ident = quote::format_ident!("{}Data", AsPascalCase(name.to_string()).to_string());
    let is_zero_copy = ix.is_zero_copy;

    // Detect any argument types that cannot be handled by bytemuck (Pod).
    // References (&[u8], &str) → need lifetime param and reference-based data setup.
    // Heap types (Vec<T>, String, Box<T>) → need Borsh serialization even in zero-copy programs.
    let has_lifetimes = ix
        .args
        .iter()
        .any(|(_, ty)| matches!(ty, syn::Type::Reference(_)));

    // A type is "non-pod" if it is a reference OR a known heap type (Vec/String/Box).
    // Non-pod types cannot use bytemuck::bytes_of and must fall back to Borsh.
    let has_non_pod_args = ix.args.iter().any(|(_, ty)| match ty {
        syn::Type::Reference(_) => true,
        syn::Type::Path(p) => {
            if let Some(seg) = p.path.segments.last() {
                let name = seg.ident.to_string();
                matches!(name.as_str(), "Vec" | "String" | "Box" | "Rc" | "Arc")
            } else {
                false
            }
        }
        _ => false,
    });

    // If any argument is non-pod, we MUST use Borsh in both backends.
    // The effective_zero_copy flag is false when heap types are present, regardless of program mode.
    let effective_zero_copy = is_zero_copy && !has_non_pod_args;

    let lifetime_param = if has_lifetimes {
        quote! { <'info> }
    } else {
        quote! {}
    };
    // Only derive Pod/Zeroable when we are ACTUALLY using the bytemuck/zero-copy path.
    // Borsh programs must never get these derives — they are dead code there and leak
    // zero-copy concepts into the Borsh binary.
    let pod_derive = if effective_zero_copy {
        quote! {
            #[derive(#naclac_path::prelude::bytemuck::Pod, #naclac_path::prelude::bytemuck::Zeroable, Clone, Copy)]
            #[bytemuck(crate = "naclac_lang::prelude::bytemuck")]
        }
    } else {
        // Borsh programs (or instructions with Vec/String args): no Pod derive.
        // For non-pod arg types we still want Clone so callers can use the struct,
        // but we skip the unsafe bytemuck impls entirely.
        quote! {
            #[derive(Clone)]
        }
    };

    // Pinocchio data setup: use bytemuck for POD types, reference-based for &[u8] args,
    // and Borsh for instructions with Vec/String args.
    let pinocchio_data_setup = if has_lifetimes {
        // Reference args (e.g. &[u8]): pack into stack buffer
        quote! {
            let mut _ix_data_buf = [0u8; 1024];
            _ix_data_buf[..8].copy_from_slice(&#disc_array);
            let mut _ix_data_len = 8;
            #(
                let arg_bytes = &#arg_names;
                _ix_data_buf[_ix_data_len.._ix_data_len + arg_bytes.len()].copy_from_slice(arg_bytes);
                _ix_data_len += arg_bytes.len();
            )*
            let ix_data_slice = &_ix_data_buf[.._ix_data_len];
        }
    } else if has_non_pod_args {
        // Heap types in Pinocchio: must use Borsh and heap-allocate (unavoidable)
        quote! {
            use #naclac_path::prelude::BorshSerialize;
            let mut _pinocchio_ix_data = #disc_array.to_vec();
            #(
                BorshSerialize::serialize(&#arg_names, &mut _pinocchio_ix_data).unwrap();
            )*
            let ix_data_slice: &[u8] = &_pinocchio_ix_data;
        }
    } else {
        // Pure POD types: zero-copy, no allocation
        quote! {
            let ix_data_struct = #ix_data_ident {
                discriminator: #disc_array,
                #( #arg_names, )*
            };
            let ix_data_slice = #naclac_path::prelude::bytemuck::bytes_of(&ix_data_struct);
        }
    };

    // Standard Solana data setup (decided at macro expansion time, no dead branches).
    let solana_data_tokens = if effective_zero_copy {
        // Stack-allocated — one final .to_vec() required by solana_program::Instruction
        quote! {
            let mut _ix_buf = [0u8; 256usize];
            _ix_buf[..8].copy_from_slice(&#disc_array);
            let mut _ix_len: usize = 8;
            #(
                let _arg_bytes = #naclac_path::prelude::bytemuck::bytes_of(&#arg_names);
                _ix_buf[_ix_len.._ix_len + _arg_bytes.len()].copy_from_slice(_arg_bytes);
                _ix_len += _arg_bytes.len();
            )*
            _ix_buf[.._ix_len].to_vec()
        }
    } else {
        // Borsh: dynamic types (Vec, String, &[u8]) or non-zero-copy programs.
        // If there are no args at all, avoid importing BorshSerialize entirely —
        // just return the discriminator vec directly.
        let has_args = !arg_names.is_empty();
        if has_args {
            quote! {
                {
                    use #naclac_path::prelude::BorshSerialize;
                    let mut _d = #disc_array.to_vec();
                    #(
                        BorshSerialize::serialize(&#arg_names, &mut _d).unwrap();
                    )*
                    _d
                }
            }
        } else {
            // No arguments: discriminator only, no Borsh machinery needed.
            quote! {
                #disc_array.to_vec()
            }
        }
    };

    quote! {
        #[repr(C)]
        #pod_derive
        pub struct #ix_data_ident #lifetime_param {
            pub discriminator: [u8; 8],
            #( pub #arg_names: #arg_types, )*
        }

        pub fn #name<'a, 'b, 'c, 'info>(
            ctx: #naclac_path::prelude::CpiContext<'a, 'b, 'c, 'info, accounts::#accounts_struct_ident<'info>>,
            #( #arg_names: #arg_types, )*
        ) -> #naclac_path::prelude::Result {
            #[cfg(not(feature = "pinocchio"))]
            let data = { #solana_data_tokens };

            let metas = ctx.accounts.to_account_metas();

            #[cfg(not(feature = "pinocchio"))]
            {
                let mut accounts: [core::mem::MaybeUninit<#naclac_path::solana_program::instruction::AccountMeta>; 16] = [core::mem::MaybeUninit::uninit(); 16];
                let mut count = 0;

                for m in metas.iter() {
                    accounts[count].write(#naclac_path::solana_program::instruction::AccountMeta {
                        pubkey: m.pubkey,
                        is_signer: m.is_signer,
                        is_writable: m.is_writable,
                    });
                    count += 1;
                }

                for m in &ctx.remaining_accounts {
                    accounts[count].write(#naclac_path::solana_program::instruction::AccountMeta {
                        pubkey: *m.key,
                        is_signer: m.is_signer,
                        is_writable: m.is_writable,
                    });
                    count += 1;
                }

                // SAFETY: We have manually initialized `count` elements in the `accounts` array.
                // Converting the partially initialized array to a slice is safe as we only
                // reference the initialized range.
                let accounts_slice = unsafe { core::slice::from_raw_parts(accounts.as_ptr() as *const #naclac_path::solana_program::instruction::AccountMeta, count) };

                let ix = #naclac_path::solana_program::instruction::Instruction {
                    program_id: *ctx.program.key,
                    accounts: accounts_slice.to_vec(),
                    data,
                };

                let mut account_infos: [core::mem::MaybeUninit<#naclac_path::prelude::AccountInfo>; 16] = [core::mem::MaybeUninit::uninit(); 16];
                let mut info_count = 0;

                account_infos[info_count].write(ctx.program.clone());
                info_count += 1;

                let infos = ctx.accounts.to_account_infos();
                for i in infos.iter() {
                    account_infos[info_count].write(i.clone());
                    info_count += 1;
                }

                for m in ctx.remaining_accounts.iter() {
                    account_infos[info_count].write(m.clone());
                    info_count += 1;
                }

                // SAFETY: We have manually initialized `info_count` elements. The memory layout
                // of `MaybeUninit<AccountInfo>` is identical to `AccountInfo`.
                let account_infos_slice = unsafe { core::slice::from_raw_parts(account_infos.as_ptr() as *const #naclac_path::prelude::AccountInfo, info_count) };

                #naclac_path::solana_program::program::invoke_signed(
                    &ix,
                    account_infos_slice,
                    ctx.signer_seeds,
                )?;
                Ok(())
            }

            #[cfg(feature = "pinocchio")]
            {
                #pinocchio_data_setup

                let mut accounts: [core::mem::MaybeUninit<#naclac_path::prelude::instruction::InstructionAccount>; 16] = [core::mem::MaybeUninit::uninit(); 16];
                let mut count = 0;

                for m in metas.iter() {
                    // SAFETY: We cast the Address pointer from the naclac Pubkey. Both have identical memory layout.
                    accounts[count].write(#naclac_path::prelude::instruction::InstructionAccount {
                        address: unsafe { &*(m.pubkey.0.as_ptr() as *const #naclac_path::prelude::Address) },
                        is_signer: m.is_signer,
                        is_writable: m.is_writable,
                    });
                    count += 1;
                }

                for m in &ctx.remaining_accounts {
                    accounts[count].write(#naclac_path::prelude::instruction::InstructionAccount {
                        address: m.view.address(),
                        is_signer: m.view.is_signer(),
                        is_writable: m.view.is_writable(),
                    });
                    count += 1;
                }

                // SAFETY: Only `count` elements are initialized. The slice represents valid initialized memory.
                let accounts_slice = unsafe { core::slice::from_raw_parts(accounts.as_ptr() as *const #naclac_path::prelude::instruction::InstructionAccount, count) };

                let ix = #naclac_path::prelude::instruction::InstructionView {
                    program_id: ctx.program.view.address(),
                    accounts: accounts_slice,
                    data: ix_data_slice,
                };

                let mut account_views: [core::mem::MaybeUninit<&#naclac_path::prelude::AccountView>; 16] = [core::mem::MaybeUninit::uninit(); 16];
                let mut view_count = 0;

                account_views[view_count].write(&ctx.program.view);
                view_count += 1;

                let infos = ctx.accounts.to_account_infos();
                for i in infos.iter() {
                    account_views[view_count].write(&i.view);
                    view_count += 1;
                }

                for m in ctx.remaining_accounts.iter() {
                    account_views[view_count].write(&m.view);
                    view_count += 1;
                }

                // SAFETY: All `view_count` references are valid and alive for the scope of the CPI call.
                let account_views_slice = unsafe { core::slice::from_raw_parts(account_views.as_ptr() as *const &#naclac_path::prelude::AccountView, view_count) };

                let mut signers: [core::mem::MaybeUninit<#naclac_path::prelude::instruction::cpi::Signer>; #naclac_path::prelude::MAX_CPI_SIGNERS] = [core::mem::MaybeUninit::uninit(); #naclac_path::prelude::MAX_CPI_SIGNERS];
                let mut seed_arrays: [[core::mem::MaybeUninit<#naclac_path::prelude::instruction::cpi::Seed>; #naclac_path::prelude::MAX_CPI_SEEDS_PER_SIGNER]; #naclac_path::prelude::MAX_CPI_SIGNERS] = [[core::mem::MaybeUninit::uninit(); #naclac_path::prelude::MAX_CPI_SEEDS_PER_SIGNER]; #naclac_path::prelude::MAX_CPI_SIGNERS];
                let mut signer_count = 0;

                for seeds in ctx.signer_seeds.iter() {
                    let mut seed_count = 0;
                    for seed in seeds.iter() {
                        seed_arrays[signer_count][seed_count].write(#naclac_path::prelude::instruction::cpi::Seed::from(*seed));
                        seed_count += 1;
                    }
                    // SAFETY: Seed slice is bound to the lifetime of the stack-allocated `seed_arrays`.
                    let seed_slice = unsafe { core::slice::from_raw_parts(seed_arrays[signer_count].as_ptr() as *const #naclac_path::prelude::instruction::cpi::Seed, seed_count) };
                    signers[signer_count].write(#naclac_path::prelude::instruction::cpi::Signer::from(seed_slice));
                    signer_count += 1;
                }

                // SAFETY: Signer slice is bound to the lifetime of the stack-allocated `signers` array.
                let signers_slice = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const #naclac_path::prelude::instruction::cpi::Signer, signer_count) };

                #naclac_path::prelude::instruction::cpi::invoke_signed_with_slice(
                    &ix,
                    account_views_slice,
                    signers_slice,
                )?;
                Ok(())
            }
        }
    }
}
