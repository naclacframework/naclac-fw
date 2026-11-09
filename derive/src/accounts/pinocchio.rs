//! The pinocchio runtime's entry point for an Accounts struct: one unsafe
//! `load_and_validate` per layout that walks the serialized accounts in the
//! entrypoint buffer itself, checking each declared slot as it goes.

use {
    super::{
        signer::signer_flag,
        variant::{Variant, Variants},
        Shared,
    },
    proc_macro2::TokenStream,
    quote::quote,
};

/// The walk over the entrypoint input shared by every layout: declared-slot
/// header checks, the fast and slow account walks and the `__views` binding.
fn account_walk(shared: &Shared) -> TokenStream {
    let n_declared_accounts = shared.fields.len();
    let declared_checks: Vec<TokenStream> = shared
        .fields
        .iter()
        .map(|f| {
            let signer = signer_flag(f, shared.generics, true);
            let writable = f.is_mut;
            let executable = f.is_executable;
            let optional = f.is_optional;
            quote! {
                {
                    let (__expected, __mask) =
                        naclac_lang::prelude::account_header_check(#signer, #writable, #executable);
                    (__expected, __mask, #optional)
                }
            }
        })
        .collect();
    let mut declared_fast_checks = shared.fields.iter().enumerate().map(|(i, field)| {
        let i = proc_macro2::Literal::usize_unsuffixed(i);
        if field.is_dup {
            quote! {
                __cursor.try_declared_dup_fast(
                    __DECLARED_CHECKS[#i].0,
                    __DECLARED_CHECKS[#i].1,
                    Self::MUT_MASK[0],
                )
            }
        } else {
            quote! {
                __cursor.try_declared_fast(__DECLARED_CHECKS[#i].0, __DECLARED_CHECKS[#i].1)
            }
        }
    });
    let needs_slow_walk = if let Some(first_fast_check) = declared_fast_checks.next() {
        let rest_fast_checks: Vec<TokenStream> = declared_fast_checks.collect();
        quote! {
            !unsafe { #first_fast_check #(&& #rest_fast_checks)* }
                || __num_accounts > #n_declared_accounts
        }
    } else {
        quote! { __num_accounts > 0 }
    };
    quote! {
        const __DECLARED_CHECKS: [(u32, u32, bool); #n_declared_accounts] =
            [#(#declared_checks),*];
        // SAFETY: `__num_accounts >= N` was checked before the walk, so every
        // declared slot exists in the input buffer; the fast checks stop at
        // the first miss, leaving `consumed` at the first slot the slow walk
        // has not yet seen; `__num_accounts <= 64` is the lookup capacity, and
        // nothing has borrowed an account yet.
        // On `Ok`, `finish_walk_slow` has filled `__lookup[..__num_accounts]`.
        if #needs_slow_walk {
            unsafe {
                __cursor.finish_walk_slow(
                    &__DECLARED_CHECKS,
                    __num_accounts,
                    &Self::MUT_MASK,
                    &Self::UNIQUE_MASK,
                )
            }?;
        }
        // SAFETY: every declared slot of `__lookup` was written, by its fast
        // check or by the slow walk, and `AccountInfo` is
        // `#[repr(transparent)]` over `AccountView`.
        let __views = unsafe {
            &*(__lookup_ptr as *const [naclac_lang::prelude::AccountType; #n_declared_accounts])
        };
    }
}

/// The check on the account count: one unsigned compare covers both bounds,
/// since `num - N` wraps past `64 - N` exactly when `num < N` or `num > 64`.
fn account_count_check(n_declared_accounts: usize) -> TokenStream {
    if n_declared_accounts <= 64 {
        let max_trailing = 64 - n_declared_accounts;
        quote! {
            if __num_accounts.wrapping_sub(#n_declared_accounts) > #max_trailing {
                return Err(naclac_lang::prelude::account_count_error(__num_accounts));
            }
        }
    } else {
        quote! {
            if __num_accounts > 64 {
                return Err(naclac_lang::prelude::NaclacError::TooManyAccountEntries.err(0));
            }
            if __num_accounts < #n_declared_accounts {
                return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
            }
        }
    }
}

fn load_and_validate_fn(shared: &Shared, cfg: TokenStream, variant: &Variant) -> TokenStream {
    let n_declared_accounts = shared.fields.len();
    let account_count_check = account_count_check(n_declared_accounts);
    let account_walk = account_walk(shared);
    let cursor_binding = if n_declared_accounts > 0 {
        quote! { let mut __cursor }
    } else {
        quote! { let __cursor }
    };
    let bumps_struct_name = &shared.bumps.struct_name;
    let bumps_instantiation = &shared.bumps.instantiation;
    let ix_deserializer = &variant.ix_deserializer;
    let field_loaders = &variant.field_loaders;
    let struct_instantiation = shared.struct_instantiation();
    let relational_checks = &shared.relational_checks;
    quote! {
        /// Loads and validates the accounts provided to the instruction.
        ///
        /// Walks the serialized accounts at `__input` itself: each declared
        /// field's signer/writable/executable requirement is checked as the
        /// account is walked, then any trailing accounts are walked into the
        /// remaining-accounts slice. Rejects more than 64 account entries
        /// (the capacity of `__lookup`) and fewer than this struct declares.
        ///
        /// # Safety
        ///
        /// `__input` must be the runtime-provided entrypoint input buffer and
        /// `__num_accounts` the account count stored at its start.
        #cfg
        pub unsafe fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
            program_id: &'static naclac_lang::prelude::Address,
            __input: *mut u8,
            __num_accounts: usize,
            __lookup: &'static mut [core::mem::MaybeUninit<naclac_lang::prelude::AccountView>; 64],
            instruction_data: &[u8],
        ) -> Result<(Self, &'static [naclac_lang::prelude::AccountType], #bumps_struct_name), naclac_lang::prelude::ProgramError> {
            #account_count_check
            let __lookup_ptr = __lookup.as_mut_ptr() as *mut naclac_lang::prelude::AccountView;
            // SAFETY: `__input` is the entrypoint buffer (caller contract) and
            // `__lookup` holds 64 slots, at least `__num_accounts` (checked above).
            #cursor_binding = unsafe {
                naclac_lang::prelude::AccountCursor::new(__input, __lookup_ptr, program_id)
            };
            #account_walk
            // Alias for compatibility with init_cpi generated code.
            let accounts = __views;
            #ix_deserializer
            let __naclac_rent = naclac_lang::prelude::LazyRent::default();
            #(#field_loaders)*
            // SAFETY: trailing accounts exist only when `__num_accounts > N`,
            // in which case the slow walk filled `__lookup[N..__num_accounts]`;
            // otherwise the slice is empty. `AccountInfo` is
            // `#[repr(transparent)]` over `AccountView`.
            let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                core::slice::from_raw_parts(
                    (__lookup_ptr as *const naclac_lang::prelude::AccountType).add(#n_declared_accounts),
                    __num_accounts.wrapping_sub(#n_declared_accounts),
                )
            };
            let mut __me = Self { #(#struct_instantiation),* };
            #(#relational_checks)*
            Ok((__me, __remaining, #bumps_instantiation))
        }
    }
}

/// The pinocchio `load_and_validate` for the zero-copy and Wincode layouts
/// (Borsh is solana-only).
pub(super) fn load_and_validate_fns(shared: &Shared, variants: &Variants) -> TokenStream {
    let zero_copy = load_and_validate_fn(
        shared,
        quote! { #[cfg(all(not(feature = "solana"), feature = "bytemuck"))] },
        &variants.zero_copy,
    );
    let wincode = load_and_validate_fn(
        shared,
        quote! { #[cfg(all(not(feature = "solana"), feature = "wincode"))] },
        &variants.wincode,
    );
    quote! {
        #zero_copy
        #wincode
    }
}
