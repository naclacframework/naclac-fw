//! The final output of `#[program]`: layout and runtime guards, the program
//! entrypoints for the solana and pinocchio runtimes, the dispatch
//! `match` over every instruction discriminator, the event-CPI receive
//! functions and the developer's module with the dispatch functions added.

use {
    super::{
        discriminators::disc_read_tokens,
        idl_build::{build_idl_assembler, IdlBuildIx},
        runtime_support::panic_support_tokens,
    },
    proc_macro2::TokenStream,
    quote::quote,
    syn::{Item, Visibility},
};

/// Everything the program output is assembled from.
pub(super) struct ProgramParts<'a> {
    pub(super) mod_vis: &'a Visibility,
    pub(super) mod_name: &'a syn::Ident,
    /// The module's items, with the dispatch functions added.
    pub(super) processed_content_items: &'a [Item],
    pub(super) ix_disc_len: usize,
    /// Each dispatch arm as `(pattern, body)`.
    pub(super) dispatch_arms: &'a [(TokenStream, TokenStream)],
    pub(super) idl_build_ixs: &'a [IdlBuildIx],
    pub(super) event_tag_bytes: &'a [u8],
    pub(super) event_cpi_receive_fns: &'a TokenStream,
}

pub(super) fn program_tokens(parts: ProgramParts) -> TokenStream {
    let ProgramParts {
        mod_vis,
        mod_name,
        processed_content_items,
        ix_disc_len,
        dispatch_arms,
        idl_build_ixs,
        event_tag_bytes,
        event_cpi_receive_fns,
    } = parts;

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

    let idl_assembler_tokens = build_idl_assembler(idl_build_ixs);
    let disc_read = disc_read_tokens(ix_disc_len);
    let panic_support = panic_support_tokens();

    quote! {
        #[cfg(not(any(feature = "bytemuck", feature = "borsh", feature = "wincode")))]
        compile_error!(
            "Naclac Error: this program declares no account-data layout. Add a `bytemuck`, \
             `borsh` or `wincode` feature forwarded to `naclac-lang` (e.g. `bytemuck = \
             [\"naclac-lang/bytemuck\"]`), enabled by default."
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
            duplicates: Option<&naclac_lang::prelude::DuplicateSlots>,
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
                    duplicates,
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
            __dups_opt: Option<&naclac_lang::prelude::DuplicateSlots>,
        ) -> naclac_lang::prelude::Result {
            if instruction_data.len() < #ix_disc_len {
                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
            }
            let instruction_discriminator = #disc_read;
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

    }
}
