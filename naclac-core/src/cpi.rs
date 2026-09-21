// ===========================================================================
// cpi.rs — Standardized Cross-Program Invocation (CPI) module
// ===========================================================================

//! # Cross-Program Invocation (CPI) Execution
//!
//! Provides the borrow-checked execution helpers for issuing Cross-Program Invocations.
//! This handles the compilation targets for Solana (via AccountInfo conversion)
//! and Pinocchio (via zero-overhead transmutations).
//!
//! `invoke`/`invoke_signed` (solana) and `invoke_pinocchio`/
//! `invoke_signed_pinocchio_handles`/`invoke_signed_pinocchio` (pinocchio)
//! always resolve to the *checked* path: on solana, the real
//! `solana_program::program::invoke_signed`, which verifies every writable
//! account's `AccountInfo` `RefCell` is actually borrowable before invoking;
//! on pinocchio, the real `solana_instruction_view::cpi::invoke_signed_with_bounds`,
//! which verifies each account's address matches the instruction's expected
//! account at that position and that its borrow state is compatible with the
//! instruction's declared mutability. Both the checked and `_unchecked`
//! pinocchio functions reject (rather than silently truncating) an account
//! list longer than `MAX_CPI_ACCOUNTS` — that bound is a bookkeeping limit,
//! not a borrow-safety one, so skipping it isn't part of what "unchecked"
//! means here. The `_unchecked` variants skip only the borrow/address
//! validation, for callers who have already established the accounts'
//! borrow-safety themselves and want to skip that CU cost — never what
//! naclac's own CPI-caller codegen generates.

/// Unified AccountMeta for cross-backend CPI usage.
#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub address: crate::prelude::Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(address: crate::prelude::Address, is_writable: bool) -> Self {
        Self {
            address,
            is_signer: false,
            is_writable,
        }
    }
    pub fn new_readonly(address: crate::prelude::Address, is_signer: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable: false,
        }
    }
}

pub trait ToAccountMetas {
    fn to_account_metas(&self) -> crate::prelude::Vec<AccountMeta>;
}

pub trait ToAccountInfos {
    fn to_account_infos(&self) -> crate::prelude::Vec<crate::prelude::AccountInfo>;
}

// ===========================================================================
// SOLANA-PROGRAM BACKEND (invoke / invoke_signed)
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub fn invoke(
    instruction: &solana_program::instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed(
    instruction: &solana_program::instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    solana_program::program::invoke_signed(instruction, &infos, signer_seeds)
}

/// Raw, no-borrow-check CPI — bypasses the `RefCell` borrow checks
/// `invoke_signed`'s real `solana_program::program::invoke_signed` performs.
/// See this module's own doc comment for when to reach for this instead.
#[cfg(not(feature = "pinocchio"))]
pub fn invoke_unchecked(
    instruction: &solana_program::instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_unchecked(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed_unchecked(
    instruction: &solana_program::instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    solana_program::program::invoke_signed_unchecked(instruction, &infos, signer_seeds)
}

// ===========================================================================
// PINOCCHIO BACKEND
// ===========================================================================

/// Bridges naclac's own `CpiHandle` to the real
/// `solana_instruction_view::cpi` (re-exported as `pinocchio::cpi`) checked
/// invoke helpers, which are generic over `AsRef<AccountView>`.
#[cfg(feature = "pinocchio")]
impl<'a> AsRef<::pinocchio::AccountView> for crate::prelude::CpiHandle<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &::pinocchio::AccountView {
        &self.info.view
    }
}

/// Converts naclac's backend-uniform `&[&[&[u8]]]` signer seeds into
/// pinocchio's own `Signer`/`Seed` types without a heap allocation, binding
/// the result to `$signers_var`. A macro rather than a function: each
/// `Signer` built here borrows from the seed buffer built in the same scope,
/// and threading that self-reference across a function boundary risks a
/// subtly wrong lifetime signature (same reasoning as
/// `system_program.rs`'s own `pinocchio_signers_from_seeds!`). Must be
/// invoked directly inside a function returning `crate::prelude::Result<()>`
/// — it returns early on overflow.
///
/// `#[macro_export]`'d so `naclac-token` (and any other downstream crate)
/// can reuse this exact, single implementation instead of hand-rolling the
/// same unsafe signer/seed-array construction independently at each CPI call
/// site — which is what `naclac-token/src/token.rs` used to do, 9 times
/// over, via raw `get_unchecked`/`as_mut_ptr().add(j)` writes into
/// `MaybeUninit` arrays (functionally the same job, more unsafe surface,
/// nine separate places to keep in sync). Every `crate::` reference inside
/// is `$crate::` instead — required for a macro invoked from a different
/// crate to still resolve against naclac-core's own `prelude`, not the
/// caller's.
#[cfg(feature = "pinocchio")]
#[macro_export]
macro_rules! cpi_signers_from_seeds {
    ($seeds:expr, $signers_var:ident) => {
        if $seeds.len() > $crate::prelude::MAX_CPI_SIGNERS {
            return Err($crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        let mut __signers: [::pinocchio::cpi::Signer; $crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut __seeds_buffer: [::pinocchio::cpi::Seed;
            $crate::prelude::MAX_CPI_SEEDS_PER_SIGNER * $crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut __seed_ranges = [(0usize, 0usize); $crate::prelude::MAX_CPI_SIGNERS];

        let mut __seed_idx = 0;

        for (i, seed_parts) in $seeds.iter().enumerate() {
            if seed_parts.len() > $crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err($crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
            let start_seed = __seed_idx;
            for part in seed_parts.iter() {
                __seeds_buffer[__seed_idx] = ::pinocchio::cpi::Seed::from(*part);
                __seed_idx += 1;
            }
            __seed_ranges[i] = (start_seed, __seed_idx);
        }

        for i in 0..$seeds.len() {
            let (start, end) = __seed_ranges[i];
            __signers[i] = ::pinocchio::cpi::Signer::from(&__seeds_buffer[start..end]);
        }

        let $signers_var: &[::pinocchio::cpi::Signer] = &__signers[..$seeds.len()];
    };
}

#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if signer_seeds.is_empty() {
        return ::pinocchio::cpi::invoke_signed_with_bounds::<
            { crate::prelude::MAX_CPI_ACCOUNTS },
            _,
        >(instruction, account_handles, &[]);
    }
    cpi_signers_from_seeds!(signer_seeds, signers);
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_handles,
        signers,
    )
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_pinocchio`/
/// `invoke_signed_pinocchio_handles`.
#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles_unchecked(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let account_infos: &[crate::prelude::AccountInfo] =
        unsafe { core::mem::transmute(account_handles) };

    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr = [const {
        core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit()
    }; crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, info) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(&info.view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    if signer_seeds.is_empty() {
        unsafe {
            ::pinocchio::cpi::invoke_unchecked(instruction, cpi_accounts);
        }
    } else {
        cpi_signers_from_seeds!(signer_seeds, signers);
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(instruction, cpi_accounts, signers);
        }
    }
    Ok(())
}

/// Checked CPI from a raw slice of `AccountView`s (rather than naclac's own
/// `CpiHandle`) — for hand-authored instruction bodies constructing an
/// `InstructionView` directly.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if signer_seeds.is_empty() {
        return ::pinocchio::cpi::invoke_signed_with_bounds::<
            { crate::prelude::MAX_CPI_ACCOUNTS },
            _,
        >(instruction, account_infos, &[]);
    }
    cpi_signers_from_seeds!(signer_seeds, signers);
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_infos,
        signers,
    )
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr = [const {
        core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit()
    }; crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, view) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    if signer_seeds.is_empty() {
        unsafe {
            ::pinocchio::cpi::invoke_unchecked(instruction, cpi_accounts);
        }
    } else {
        cpi_signers_from_seeds!(signer_seeds, signers);
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(instruction, cpi_accounts, signers);
        }
    }
    Ok(())
}

#[cfg(all(kani, feature = "pinocchio"))]
mod kani_proofs {
    use crate::prelude::{MAX_CPI_SEEDS_PER_SIGNER, MAX_CPI_SIGNERS};

    /// Proves `cpi_signers_from_seeds!` rejects `> MAX_CPI_SIGNERS` signer
    /// groups with a clean `TooManyCpiSigners` error rather than truncating
    /// silently — this exact class of bug (silent truncation instead of
    /// erroring) was found and fixed once already in
    /// `system_program.rs`'s sibling macro (see docs/plan/kani-audit.md);
    /// this proves the fix actually holds for the shared macro every CPI
    /// call site in the workspace now goes through.
    #[kani::proof]
    fn prove_rejects_too_many_signers() -> crate::prelude::Result<()> {
        let empty: &[&[u8]] = &[];
        // One more than the real limit — every real limit is small (4), so
        // a fixed array one longer than that is cheap to build directly.
        let too_many: [&[&[u8]]; MAX_CPI_SIGNERS + 1] = [empty; MAX_CPI_SIGNERS + 1];
        let signer_seeds: &[&[&[u8]]] = &too_many;
        let result: crate::prelude::Result<()> = (|| {
            cpi_signers_from_seeds!(signer_seeds, _signers);
            Ok(())
        })();
        assert!(
            result.is_err(),
            "more than MAX_CPI_SIGNERS signer groups must be rejected, not silently truncated"
        );
        Ok(())
    }

    /// Same proof for the per-signer seed-count limit.
    #[kani::proof]
    fn prove_rejects_too_many_seeds_per_signer() -> crate::prelude::Result<()> {
        let one_seed: &[u8] = &[0u8];
        let too_many_seeds: [&[u8]; MAX_CPI_SEEDS_PER_SIGNER + 1] =
            [one_seed; MAX_CPI_SEEDS_PER_SIGNER + 1];
        let signer_seeds: &[&[&[u8]]] = &[&too_many_seeds];
        let result: crate::prelude::Result<()> = (|| {
            cpi_signers_from_seeds!(signer_seeds, _signers);
            Ok(())
        })();
        assert!(
            result.is_err(),
            "more than MAX_CPI_SEEDS_PER_SIGNER seeds in one group must be rejected, not silently truncated"
        );
        Ok(())
    }

    /// Proves the macro never panics and produces a correctly-shaped
    /// `signers` slice (one `Signer` per signer group, in order) for any
    /// symbolic seed *bytes* within a small, bounded, realistic shape (2
    /// signer groups, 2 seed parts each, 4 bytes per part) — deliberately
    /// modest, per the same "keep buffers/unwind bounds small" lesson from
    /// `external_plugin_registry`'s earlier resource blowup, since this
    /// exercises the same macro body's loops regardless of exact
    /// signer/seed count.
    #[kani::proof]
    fn prove_within_bounds_is_correct() -> crate::prelude::Result<()> {
        let seed_a0: [u8; 4] = kani::any();
        let seed_a1: [u8; 4] = kani::any();
        let seed_b0: [u8; 4] = kani::any();
        let seed_b1: [u8; 4] = kani::any();
        let signer_a: [&[u8]; 2] = [&seed_a0, &seed_a1];
        let signer_b: [&[u8]; 2] = [&seed_b0, &seed_b1];
        let signer_seeds: &[&[&[u8]]] = &[&signer_a, &signer_b];

        cpi_signers_from_seeds!(signer_seeds, signers);
        assert_eq!(
            signers.len(),
            signer_seeds.len(),
            "must produce exactly one Signer per input signer group"
        );
        Ok(())
    }
}
