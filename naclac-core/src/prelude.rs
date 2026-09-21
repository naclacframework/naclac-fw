// ===========================================================================
// prelude.rs — Naclac unified prelude
//
// Feature routing:
//   default ("solana")       → solana_program backend (std, lifetimes) — NO borsh by default
//   "solana" + "borsh"       → solana_program + borsh serialization (Account<T>, #[component])
//   "zero-copy"              → Account<T> zero-copy branch, zero-copy events via bytemuck::Pod
//   "pinocchio"              → pinocchio backend (no_std, no lifetimes, zero-copy)
//
// The two backends are mutually exclusive. Enabling "pinocchio" hides every
// solana_program symbol and replaces it with the pinocchio equivalents.
// ===========================================================================

pub use crate::context::{Bumps, Context, LoadableAccounts, ValidationResult};
#[cfg(feature = "pinocchio")]
pub use crate::cpi::{invoke_signed_pinocchio, invoke_signed_pinocchio_unchecked};
pub use crate::cpi::{self, AccountMeta, ToAccountMetas};
pub use naclac_macros::*;

// --- Wrappers: common types (shared by both backends) ---
pub use crate::error::NaclacError;

// These traits/types exist in wrappers for BOTH Solana and Pinocchio backends
pub use crate::wrappers::{
    Account, AsRefByteSlice, AssociatedToken, CpiHandle, CpiHandleMut, Discriminator, Id, Ids,
    Interface, InterfaceAccount, NaclacAccount, NaclacZeroCopy, Owner, Program, Signer, Span,
    System, ToAccountInfo, ToAddress, ToCpiHandle, ToCpiHandleMut, Token, Token2022,
    TokenInterface, ValidateInterfaceLayout, ZcString,
};

pub type ZcVec<T> = Span<T>;

pub use crate::system_program::{CreateAccountAccounts, SystemTransferAccounts};

pub use crate::realloc::resize_with_rent;
pub use crate::realloc::const_rent_lamports;

// ---------------------------------------------------------------------------
// CPI Stack-Allocation Limits
//
// These constants control the maximum number of PDA signers and seeds that
// the generated CPI helpers will support via stack-allocated arrays.
// They are intentionally conservative to keep stack usage well under the
// Solana 4 KB limit.
//
// ⚠️  WARNING: Raising these values increases stack frame size.
//     Each additional signer costs `MAX_CPI_SEEDS_PER_SIGNER * 16` bytes.
//     Do NOT exceed values that would push total stack usage past 4 KB.
//
// To override globally in your crate, re-export your own constants from a
// prelude wrapper before including naclac_lang.
// ---------------------------------------------------------------------------

/// Maximum number of PDA signers supported per CPI call.
/// Default: 4. Each signer holds up to `MAX_CPI_SEEDS_PER_SIGNER` seeds.
pub const MAX_CPI_SIGNERS: usize = 4;

/// Maximum number of seeds per PDA signer in a CPI call.
/// Default: 16. Solana itself enforces a limit of 16 seeds per PDA.
pub const MAX_CPI_SEEDS_PER_SIGNER: usize = 16;

/// Maximum number of accounts the pinocchio-backend CPI helpers
/// (`cpi::invoke_pinocchio`/`invoke_signed_pinocchio_handles`/
/// `invoke_signed_pinocchio` and their `_unchecked` counterparts) will
/// accept in one call — a stack-allocated array bound. Exceeding it is a
/// hard error, not a silent truncation.
pub const MAX_CPI_ACCOUNTS: usize = 32;

/// `true` if `fixed + extra` (a CPI call's fixed accounts plus a
/// caller-supplied `remaining_accounts` count) both fits in a `usize` and
/// is `<= max` — the shared, checked form of the `fixed + extra > max`
/// pattern several pinocchio-backend CPI builders use to size their
/// stack-allocated account arrays before writing into them (e.g.
/// `naclac-metadata`'s `execute.rs::execute_signed`). Returns `false` on
/// overflow rather than panicking or wrapping, so a caller never mistakes
/// an overflowed sum for a small, in-bounds one.
pub const fn cpi_account_count_fits(fixed: usize, extra: usize, max: usize) -> bool {
    match fixed.checked_add(extra) {
        Some(total) => total <= max,
        None => false,
    }
}

#[cfg(kani)]
mod cpi_account_count_kani_proofs {
    use super::*;

    /// Proves `cpi_account_count_fits` never panics for any input
    /// (including values that would overflow `fixed + extra`) and is
    /// correct: it returns `true` exactly when the real, non-overflowing
    /// sum is `<= max`, and `false` whenever the addition would overflow —
    /// never silently treating an overflowed sum as small and in-bounds.
    #[kani::proof]
    fn prove_cpi_account_count_fits_never_panics_and_is_correct() {
        let fixed: usize = kani::any();
        let extra: usize = kani::any();
        let max: usize = kani::any();

        let result = cpi_account_count_fits(fixed, extra, max);
        match fixed.checked_add(extra) {
            Some(total) => assert_eq!(result, total <= max),
            None => assert!(!result, "an overflowing sum must never be reported as fitting"),
        }
    }
}

// `ToAccountInfos` is implemented for both branches of `Account<T>`/
// `InterfaceAccount<T>` — only absent under pinocchio, where the trait
// itself has no `to_account_infos` method to begin with.
#[cfg(not(feature = "pinocchio"))]
pub use crate::wrappers::ToAccountInfos;

// Pinocchio-only: AccountView is already exported from the pinocchio block below (line ~159)
// Do NOT re-export it from wrappers here — that creates a circular private import

// --- Module Routing (core vs std) ---
#[cfg(any(feature = "pinocchio", feature = "no-std"))]
pub use core::fmt;

#[cfg(not(any(feature = "pinocchio", feature = "no-std")))]
pub use std::fmt;

// --- Alloc Routing (Vec, String, etc.) ---
// In no_std environments (Pinocchio), alloc must be linked explicitly.
// The full entrypoint path calls pinocchio::default_allocator!() which registers one.
#[cfg(any(feature = "no-std", feature = "pinocchio"))]
extern crate alloc;

#[cfg(any(feature = "no-std", feature = "pinocchio"))]
pub use alloc::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

#[cfg(not(any(feature = "no-std", feature = "pinocchio")))]
pub use std::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

// --- IO Routing ---
#[cfg(all(feature = "no-std", not(feature = "pinocchio")))]
pub use borsh::io;

#[cfg(not(any(feature = "pinocchio", feature = "no-std")))]
pub use std::io;

// --- Bytemuck (both backends need this for zero-copy) ---
pub use crate::bytemuck;
pub use crate::bytemuck::{Pod, Zeroable};

// ===========================================================================
// SOLANA-PROGRAM BACKEND  (feature = "solana")
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_program;
#[cfg(not(feature = "pinocchio"))]
pub use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    log::sol_log_data,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    sysvar::rent::Rent,
    sysvar::Sysvar,
};

#[cfg(not(feature = "pinocchio"))]
pub use solana_system_interface::program::ID as SYSTEM_PROGRAM_ID;

#[cfg(not(feature = "pinocchio"))]
pub use solana_program::msg;

#[cfg(not(feature = "pinocchio"))]
pub type NaclacResult<T = (), E = solana_program::program_error::ProgramError> =
    core::result::Result<T, E>;
#[cfg(not(feature = "pinocchio"))]
pub use NaclacResult as Result;

#[cfg(not(feature = "pinocchio"))]
pub use solana_address;
#[cfg(not(feature = "pinocchio"))]
pub use solana_address::Address;

// --- OFFICIAL PROGRAM IDS ---
#[cfg(not(feature = "pinocchio"))]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address =
    solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
#[cfg(not(feature = "pinocchio"))]
pub const TOKEN_PROGRAM_ID: Address =
    solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
#[cfg(not(feature = "pinocchio"))]
pub const TOKEN_2022_PROGRAM_ID: Address =
    solana_address::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
#[cfg(not(feature = "pinocchio"))]
pub const RENT_SYSVAR_ID: Address =
    solana_address::address!("SysvarRent111111111111111111111111111111111");

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh;

#[cfg(not(feature = "pinocchio"))]
pub use crate::base58;
#[cfg(not(feature = "pinocchio"))]
pub use crate::system_program;
#[cfg(not(feature = "pinocchio"))]
#[cfg(not(feature = "pinocchio"))]
pub use solana_system_interface;

#[cfg(not(feature = "pinocchio"))]
#[derive(Clone)]
#[repr(C)]
pub struct AccountInfo {
    pub key: &'static Address,
    pub lamports: std::rc::Rc<core::cell::RefCell<&'static mut u64>>,
    pub data: std::rc::Rc<core::cell::RefCell<&'static mut [u8]>>,
    pub owner: &'static Address,
    pub _unused: u64,
    pub is_signer: bool,
    pub is_writable: bool,
    pub executable: bool,
}

#[cfg(not(feature = "pinocchio"))]
impl AccountInfo {
    pub fn data_is_empty(&self) -> bool {
        self.data.borrow().is_empty()
    }

    pub fn try_borrow_data(
        &self,
    ) -> Result<core::cell::Ref<'_, [u8]>, solana_program::program_error::ProgramError> {
        self.data
            .try_borrow()
            .map(|r| core::cell::Ref::map(r, |d| &**d))
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))
    }

    pub fn try_borrow_mut_data(
        &self,
    ) -> Result<core::cell::RefMut<'_, [u8]>, solana_program::program_error::ProgramError> {
        self.data
            .try_borrow_mut()
            .map(|r| core::cell::RefMut::map(r, |d| &mut **d))
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))
    }

    /// # Safety
    ///
    /// This function transmutes an `AccountInfo` with a specific lifetime into a lifetime-erased representation.
    /// The caller must ensure that the source lifetime is valid for the duration of the erased type's usage.
    #[inline(always)]
    pub unsafe fn from_lifetime<'info>(
        info: solana_program::account_info::AccountInfo<'info>,
    ) -> Self {
        core::mem::transmute(info)
    }

    /// # Safety
    ///
    /// This function transmutes the lifetime-erased `AccountInfo` back to a specific lifetime `'info`.
    /// The caller must ensure that the lifetime `'info` is valid for the lifetime-erased representation.
    #[inline(always)]
    pub unsafe fn to_lifetime<'info>(&self) -> solana_program::account_info::AccountInfo<'info> {
        core::mem::transmute(self.clone())
    }

    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        let mut lamports = self
            .lamports
            .try_borrow_mut()
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))?;
        **lamports = lamports
            .checked_sub(amount)
            .ok_or(crate::error::NaclacError::InsufficientFunds.err(0))?;
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut lamports = self
            .lamports
            .try_borrow_mut()
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))?;
        **lamports = lamports
            .checked_add(amount)
            .ok_or(crate::error::NaclacError::ArithmeticOverflow.err(0))?;
        Ok(())
    }

    pub fn lamports(&self) -> u64 {
        **self.lamports.borrow()
    }

    pub fn assign(&self, new_owner: &Address) {
        unsafe {
            let solana_info = self.to_lifetime();
            solana_info.assign(core::mem::transmute::<
                &Address,
                &solana_program::pubkey::Pubkey,
            >(new_owner));
        }
    }

    pub fn address(&self) -> Address {
        *self.key
    }

    /// Resize the account's data, delegating to the real
    /// `solana_program::account_info::AccountInfo::resize` (an inherent
    /// `&self` method backed by unsafe raw-pointer manipulation into
    /// runtime memory) via the same `to_lifetime()` cast already used by
    /// `assign()` above.
    pub fn resize(&self, new_len: usize) -> Result<()> {
        unsafe { self.to_lifetime().resize(new_len) }
            .map_err(|_| crate::error::NaclacError::InvalidRealloc.err(0))
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh::{BorshDeserialize, BorshSerialize};

/// Helper: get unix timestamp from the Clock sysvar (solana_program only)
#[inline(always)]
#[cfg(not(feature = "pinocchio"))]
pub fn unix_timestamp() -> Result<i64> {
    Clock::get().map(|c| c.unix_timestamp)
}

/// Helper: get unix timestamp from the Clock sysvar (pinocchio only)
#[inline(always)]
#[cfg(feature = "pinocchio")]
pub fn unix_timestamp() -> Result<i64> {
    Ok(Clock::get()?.unix_timestamp)
}

// ===========================================================================
// PINOCCHIO BACKEND  (feature = "pinocchio")
// ===========================================================================
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio;

#[cfg(feature = "pinocchio")]
pub use pinocchio::{
    error::ProgramError,
    instruction,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, Address as PinocchioAddress, ProgramResult,
};

pub use crate::cursor::{mut_mask_set_bit, AccountBitvec};
#[cfg(feature = "pinocchio")]
pub use crate::cursor::{parse_ix_context, AccountCursor};

#[cfg(feature = "pinocchio")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Address(pub [u8; 32]);

#[cfg(feature = "pinocchio")]
impl From<PinocchioAddress> for Address {
    #[inline(always)]
    fn from(address: PinocchioAddress) -> Self {
        // SAFETY: PinocchioAddress is [u8; 32], same as Address([u8; 32]).
        // ptr::copy_nonoverlapping of exactly 32 bytes lets LLVM inline
        // 4 × u64 register stores instead of emitting a sol_memcpy_ call.
        unsafe { core::mem::transmute(address) }
    }
}

#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Zeroable for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Pod for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::ZeroableInOption for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::PodInOption for Address {}

/// Real Solana protocol limit on the number of seeds a PDA derivation may
/// use (`solana_program::pubkey::MAX_SEEDS`) — mirrored here, not invented,
/// since `derive_program_address` below needs a fixed-size scratch buffer
/// (no heap allocation, so it works under pinocchio's `no_std`).
pub const MAX_PDA_SEEDS: usize = 16;

/// Derives a PDA address for arbitrary `seeds` under `program_id` at the
/// given `bump`, via a single hash-and-compare (`create_program_address`-
/// equivalent) rather than an on-chain `find_program_address` bump search —
/// naclac never runs the search loop on-chain, so `bump` must already be
/// known by the caller. The one shared implementation every PDA-verifying
/// code path in naclac uses: the `#[account(seeds = [...], bump = ...)]`
/// constraint naclac-macros generates and `associated_token::derive_ata_address`
/// both call this internally rather than each hashing independently. It's
/// also the function hand-written instruction code needs to verify an
/// arbitrary `AccountInfo` — e.g. one of `ctx.remaining_accounts` — against
/// an expected PDA, something no declarative `#[account(...)]` constraint
/// can reach (those only attach to a named struct field).
///
/// Panics if `seeds.len() > MAX_PDA_SEEDS` — the same real protocol limit
/// `find_program_address`/`create_program_address` themselves enforce, not
/// an invented restriction.
/// Fills a `[&[u8]; MAX_PDA_SEEDS + 3]` scratch buffer with `seeds` followed
/// by `bump_slice`, `program_id`'s bytes, and the `"ProgramDerivedAddress"`
/// domain tag, returning `(scratch, n)` where `scratch[..n]` is the real
/// hash input — shared by `derive_program_address` and
/// `find_program_address`, which otherwise duplicated this exact
/// index-tracking loop independently. Caller must ensure `seeds.len() <=
/// MAX_PDA_SEEDS` (both callers `assert!` this immediately before calling).
fn build_pda_hash_inputs<'a>(
    seeds: &[&'a [u8]],
    bump_slice: &'a [u8],
    program_id: &'a Address,
) -> ([&'a [u8]; MAX_PDA_SEEDS + 3], usize) {
    let mut scratch: [&[u8]; MAX_PDA_SEEDS + 3] = [&[]; MAX_PDA_SEEDS + 3];
    let mut n = 0;
    for seed in seeds {
        scratch[n] = seed;
        n += 1;
    }
    scratch[n] = bump_slice;
    n += 1;
    scratch[n] = program_id.as_ref();
    n += 1;
    scratch[n] = b"ProgramDerivedAddress";
    n += 1;
    (scratch, n)
}

pub fn derive_program_address(seeds: &[&[u8]], bump: u8, program_id: &Address) -> Address {
    assert!(
        seeds.len() <= MAX_PDA_SEEDS,
        "derive_program_address: too many seeds (max {MAX_PDA_SEEDS})"
    );

    let bump_arr = [bump];
    let (scratch, n) = build_pda_hash_inputs(seeds, &bump_arr[..], program_id);
    let inputs = &scratch[..n];

    #[cfg(not(feature = "pinocchio"))]
    {
        let hash_result = solana_program::hash::hashv(inputs);
        Address::new_from_array(hash_result.to_bytes())
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut hash_result = [0u8; 32];
        // SAFETY: `sol_sha256` is a native Solana SBF syscall; `inputs`
        // outlives the call and each slice element is a valid (ptr, len)
        // pair, matching the syscall's expected array-of-`SolBytes` layout.
        unsafe {
            solana_define_syscall::definitions::sol_sha256(
                inputs.as_ptr() as *const u8,
                inputs.len() as u64,
                hash_result.as_mut_ptr(),
            );
        }
        Address::new_from_array(hash_result)
    }
}

/// Finds the canonical off-curve PDA for `seeds` under `program_id`, the same
/// answer real `find_program_address` would give, via `sol_sha256` +
/// `sol_curve_validate_point` called directly rather than the native
/// `sol_try_find_program_address` syscall — Quasar's `based_try_find_program_address`
/// and Anchor v2's `find_and_verify_program_address` both use this same pair
/// of primitives instead of the all-in-one native syscall because it measures
/// cheaper per attempt (naclac's own bench: ~1,530 CU/attempt via the native
/// syscall vs ~550 CU/attempt via this pair, on the pinocchio backend).
///
/// Unlike `derive_program_address` above (which only checks a caller-supplied
/// bump for self-consistency), this performs the real, bounded 256-iteration
/// search naclac otherwise bans on-chain — the one place naclac still needs
/// it: establishing that a fresh PDA under dynamic (non-literal) seeds is
/// canonical at the moment it's created, since no compile-time precomputation
/// is possible for seeds that aren't known until runtime.
///
/// Panics if `seeds.len() > MAX_PDA_SEEDS`, matching `derive_program_address`.
/// Returns `NaclacError::ConstraintSeeds` in the cryptographically
/// unreachable case where no bump in 0..=255 lands off-curve.
pub fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> Result<(Address, u8)> {
    assert!(
        seeds.len() <= MAX_PDA_SEEDS,
        "find_program_address: too many seeds (max {MAX_PDA_SEEDS})"
    );

    let mut bump_arr = [255u8];
    // Raw-pointer-derived slice, not `&bump_arr[..]`: the loop below mutates
    // `bump_arr` on every iteration while `scratch[bump_idx]` still holds a
    // reference into it from the previous iteration, which a safe `&[u8]`
    // borrow can't express (the borrow checker can't prove per-element array
    // liveness precisely enough here) — Quasar's `based_try_find_program_address`
    // hits the same wall and resolves it the same way.
    //
    // SAFETY: `bump_ptr` stays valid for `bump_arr`'s lifetime (both are
    // function-local and `bump_arr` is never moved). The slice this produces
    // is only ever read by the syscalls below as a raw `(ptr, len)` pair —
    // never through a real Rust reference — so mutating the byte behind it
    // between iterations via `bump_ptr.write` is not a live-reference aliasing
    // violation, just sequential, single-threaded byte reuse.
    let bump_ptr = bump_arr.as_mut_ptr();
    let bump_slice: &[u8] = unsafe { core::slice::from_raw_parts(bump_ptr, 1) };

    let (scratch, total) = build_pda_hash_inputs(seeds, bump_slice, program_id);
    let inputs = &scratch[..total];

    const CURVE25519_EDWARDS: u64 = 0;
    let mut bump: i16 = 255;
    while bump >= 0 {
        // SAFETY: same invariant as `bump_slice`'s construction above —
        // `scratch[bump_idx]` already points at this exact byte; writing
        // through the raw pointer changes what the next syscall call reads
        // without needing to re-borrow or re-store the slice.
        unsafe { bump_ptr.write(bump as u8) };

        #[cfg(not(feature = "pinocchio"))]
        let hash_bytes: [u8; 32] = solana_program::hash::hashv(inputs).to_bytes();

        #[cfg(feature = "pinocchio")]
        let hash_bytes: [u8; 32] = {
            let mut out = [0u8; 32];
            // SAFETY: same syscall, same argument shape as `derive_program_address` above.
            unsafe {
                solana_define_syscall::definitions::sol_sha256(
                    inputs.as_ptr() as *const u8,
                    inputs.len() as u64,
                    out.as_mut_ptr(),
                );
            }
            out
        };

        #[cfg(not(feature = "pinocchio"))]
        // SAFETY: `sol_curve_validate_point` is a native Solana SBF syscall;
        // `hash_bytes` is a valid 32-byte buffer for the duration of the call.
        // Returns 0 if the point is a valid curve point (on-curve, invalid
        // PDA), non-zero if off-curve (valid PDA) — same convention Quasar
        // and Anchor v2's own usage of this syscall document.
        let off_curve = unsafe {
            solana_define_syscall::definitions::sol_curve_validate_point(
                CURVE25519_EDWARDS,
                hash_bytes.as_ptr(),
                core::ptr::null_mut(),
            ) != 0
        };

        #[cfg(feature = "pinocchio")]
        // SAFETY: same syscall and convention as the non-pinocchio branch above.
        let off_curve = unsafe {
            solana_define_syscall::definitions::sol_curve_validate_point(
                CURVE25519_EDWARDS,
                hash_bytes.as_ptr(),
                core::ptr::null_mut(),
            ) != 0
        };

        if off_curve {
            return Ok((Address::new_from_array(hash_bytes), bump as u8));
        }
        bump -= 1;
    }

    Err(NaclacError::ConstraintSeeds.into())
}

#[cfg(kani)]
mod pda_kani_proofs {
    use super::*;

    /// Proves `build_pda_hash_inputs` never panics for any `seeds.len() <=
    /// MAX_PDA_SEEDS` — the real, naclac-authored logic behind both
    /// `derive_program_address` and `find_program_address`'s scratch-buffer
    /// construction, extracted from both so it's provable once instead of
    /// duplicated and unprovable in each (see docs/plan/kani-audit.md).
    /// Correctness, not just panic-freedom: `n` must equal exactly
    /// `seeds.len() + 3` (every seed, plus bump/program_id/domain-tag), and
    /// `scratch[..n]` must contain those inputs in the documented order —
    /// a wrong index or dropped element here would silently derive the
    /// wrong PDA for every account in the framework.
    // Every `scratch[i]` slot is a direct reference passthrough
    // (`scratch[n] = seed`, never a copy or transform) — so the only
    // genuinely meaningful property is that the *same reference* landed in
    // the *right slot*, which `core::ptr::eq` proves directly and cheaply
    // for any element size, no `memcmp`/unwind-bound tuning needed. An
    // earlier version used byte-content `assert_eq!` instead, which pulls
    // in `memcmp` (and its own loop, needing its own unwind budget) for no
    // real gain: content equality is structurally guaranteed by the source
    // being a plain reference copy, so it wasn't proving anything
    // `core::ptr::eq` doesn't already prove more directly.
    #[kani::proof]
    fn prove_build_pda_hash_inputs_within_limit_is_correct() {
        let s0: [u8; 4] = kani::any();
        let s1: [u8; 4] = kani::any();
        let seeds: [&[u8]; 2] = [&s0, &s1];
        let bump_arr = [kani::any::<u8>()];
        let program_id_bytes: [u8; 32] = kani::any();
        let program_id = Address::new_from_array(program_id_bytes);

        let (scratch, n) = build_pda_hash_inputs(&seeds, &bump_arr[..], &program_id);

        assert_eq!(n, seeds.len() + 3, "n must count every seed plus bump/program_id/domain-tag");
        assert!(core::ptr::eq(scratch[0].as_ptr(), s0.as_ptr()) && scratch[0].len() == s0.len());
        assert!(core::ptr::eq(scratch[1].as_ptr(), s1.as_ptr()) && scratch[1].len() == s1.len());
        assert!(
            core::ptr::eq(scratch[2].as_ptr(), bump_arr.as_ptr())
                && scratch[2].len() == bump_arr.len()
        );
        let program_id_ref = program_id.as_ref();
        assert!(
            core::ptr::eq(scratch[3].as_ptr(), program_id_ref.as_ptr())
                && scratch[3].len() == program_id_ref.len()
        );
        // scratch[4] (the domain-tag slot) has no caller-owned variable to
        // compare pointer identity against — unlike the slots above, it's
        // never derived from symbolic input at all (unconditionally
        // `scratch[n] = b"ProgramDerivedAddress"` in the source), so there's
        // nothing here for a proof to meaningfully distinguish "right" from
        // "wrong" on. Not asserted.
    }

    /// Proves the `n` index this function builds up (seeds + 3 fixed
    /// entries) never exceeds the `MAX_PDA_SEEDS + 3`-sized `scratch`
    /// array, for the full real range of `seeds.len()` (0..=16), not just
    /// the 2-seed case the correctness proof above uses for tractability.
    #[kani::proof]
    #[kani::unwind(20)]
    fn prove_build_pda_hash_inputs_never_overflows_scratch_at_max_seeds() {
        let seed_bytes: [u8; 1] = kani::any();
        let seed: &[u8] = &seed_bytes;
        let num_seeds: usize = kani::any();
        kani::assume(num_seeds <= MAX_PDA_SEEDS);
        let seeds_storage = [seed; MAX_PDA_SEEDS];
        let seeds = &seeds_storage[..num_seeds];

        let bump_arr = [0u8];
        let program_id = Address::new_from_array([0u8; 32]);
        let (_scratch, n) = build_pda_hash_inputs(seeds, &bump_arr[..], &program_id);
        assert!(n <= MAX_PDA_SEEDS + 3);
    }

    /// **Cannot pass as written — kept as documented evidence of a genuine
    /// Kani tooling limitation, not a bug to fix or a slow proof to wait
    /// out.** Attempting to exercise `derive_program_address`'s real
    /// `solana_program::hash::hashv` call hits `TerminatorKind::InlineAsm is
    /// not currently supported by Kani` inside
    /// `std::arch::x86_64::__cpuid_count` — the `sha2` crate this depends on
    /// does runtime CPU-feature detection via literal `cpuid` inline
    /// assembly (to pick hardware-accelerated vs. software SHA), and Kani's
    /// MIR-to-GOTO translator cannot model inline assembly on any target,
    /// regardless of time or compute budget. Unlike the `u128`-arithmetic
    /// proofs elsewhere in this audit (genuinely slow, but CI can finish
    /// them given enough time), raising CI compute does not help here —
    /// this is the same class of hard wall `find_program_address` already
    /// hits via its `sol_curve_validate_point` syscall dependency, just
    /// reached through a different path. The provable subset of this
    /// function's own logic is `build_pda_hash_inputs` above, which is
    /// already fully proven; the hash call itself is third-party crate
    /// behavior outside naclac's own code to verify.
    #[kani::proof]
    #[kani::unwind(80)]
    fn prove_derive_program_address_never_panics() {
        let s0: [u8; 4] = kani::any();
        let seeds: [&[u8]; 1] = [&s0];
        let bump: u8 = kani::any();
        let program_id_bytes: [u8; 32] = kani::any();
        let program_id = Address::new_from_array(program_id_bytes);
        let _ = derive_program_address(&seeds, bump, &program_id);
    }

    // `find_program_address` has no end-to-end proof here, deliberately:
    // every code path through it calls `sol_curve_validate_point`, a native
    // Solana syscall declared `extern "C"` with no body — Kani cannot
    // execute a foreign function it has no implementation for, so any
    // harness that actually reaches that call fails with "not currently
    // supported by Kani" regardless of how much time or CI budget is spent
    // on it. This is a hard tooling limitation, not a resource/speed
    // problem like the `u128`/hash cases above. The provable subset of this
    // function's own logic (the scratch-buffer construction) is exactly
    // `build_pda_hash_inputs`, already proven above since both functions
    // share it.
}

/// Trait for Naclac-compatible POD types (including Option<T> support)
pub trait NaclacPod: Sized {
    fn naclac_from_bytes(data: &[u8]) -> Self;
    fn naclac_size() -> usize;
}

/// Reads a value's own in-memory bytes without requiring `bytemuck::Pod`.
///
/// `Pod` (and `bytemuck::bytes_of`) is required for the *reverse* direction
/// — interpreting arbitrary, possibly-invalid bytes as a value — which is
/// why a data-carrying zero-copy `#[defined_type]` enum deliberately gets
/// only `CheckedBitPattern`, not `Pod` (some byte patterns, like an unknown
/// discriminant, aren't valid values of the type). Going the other way —
/// reading the bytes of a value that already exists and is therefore
/// already valid — carries no such risk and is sound for any `Copy` type
/// regardless of whether it's `Pod`, since no new value is ever constructed
/// from unchecked bytes here.
#[inline(always)]
pub fn bytes_of_checked_bit_pattern<T: Copy>(value: &T) -> &[u8] {
    // SAFETY: `value` is a `&T` to an already-valid, already-constructed
    // `T`, so viewing its own `size_of::<T>()` bytes as a `&[u8]` reads
    // memory that unquestionably belongs to `value` and is already
    // initialized — it doesn't construct a `T` from these bytes, only the
    // reverse (an established `T` yielding its own bytes), so `T: Pod` is
    // not required.
    unsafe { core::slice::from_raw_parts((value as *const T).cast::<u8>(), core::mem::size_of::<T>()) }
}

/// The padding gap needed after `offset` bytes to reach the next
/// `align`-aligned boundary — `0` if already aligned. Real callers always
/// pass a real `core::mem::align_of::<T>()` (a power of two, always `>= 1`
/// for any real Rust type), never an arbitrary `align`; this proves the
/// function is safe and correct within that actual contract, not merely
/// assumed. Previously duplicated as identical generated tokens in two
/// places in `naclac-macros/src/pod_struct_checks.rs` (the padding
/// computation for every zero-copy `#[component]`/`defined_type` struct's
/// generated layout) — extracted here so it's provable once instead of
/// trusted identical twice over (see docs/plan/kani-audit.md).
pub const fn compute_padding_gap(offset: usize, align: usize) -> usize {
    let rem = offset % align;
    if rem == 0 {
        0
    } else {
        align - rem
    }
}

#[cfg(kani)]
mod padding_kani_proofs {
    use super::*;

    /// Proves `compute_padding_gap` never panics for any `align >= 1` (the
    /// real contract — `align_of::<T>()` is never 0 for a real type) and
    /// that the result is actually correct: `offset + gap` lands exactly on
    /// an `align`-aligned boundary, and `gap` is the smallest such value
    /// (`< align`) — not just "doesn't panic", since a wrong gap here would
    /// silently corrupt the generated memory layout of every zero-copy
    /// `#[component]`/`defined_type` struct in the framework.
    #[kani::proof]
    fn prove_compute_padding_gap_within_contract_is_correct() {
        let offset: usize = kani::any();
        let align: usize = kani::any();
        kani::assume(align >= 1);
        kani::assume(align <= 64); // real alignments are small powers of two

        let gap = compute_padding_gap(offset, align);
        assert!(gap < align, "gap must always be smaller than align");
        if let Some(padded) = offset.checked_add(gap) {
            assert_eq!(padded % align, 0, "offset + gap must land on an aligned boundary");
        }
    }
}

#[macro_export]
macro_rules! impl_naclac_pod {
    ($($t:ty),*) => {
        $(
            impl $crate::prelude::NaclacPod for $t {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // SAFETY: We expect the data to be the correct size and valid for the type.
                    // We use read_unaligned to avoid alignment issues in zero-copy buffers.
                    unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<$t>()
                }
            }
        )*
    };
}
pub use impl_naclac_pod;

impl<T: NaclacPod> NaclacPod for Option<T> {
    #[inline(always)]
    fn naclac_from_bytes(data: &[u8]) -> Self {
        if data[0] == 0 {
            None
        } else {
            Some(T::naclac_from_bytes(&data[1..1 + T::naclac_size()]))
        }
    }
    #[inline(always)]
    fn naclac_size() -> usize {
        1 + T::naclac_size()
    }
}

impl_naclac_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Bool, Address);

// Generic over `N` rather than hand-listed sizes (the macro above only
// covers exact types) — any `[u8; N]` fixed-size instruction arg or event
// field works, not just the specific lengths someone happened to enumerate.
impl<const N: usize> NaclacPod for [u8; N] {
    #[inline(always)]
    fn naclac_from_bytes(data: &[u8]) -> Self {
        // SAFETY: same contract as `impl_naclac_pod!`'s generated impls —
        // `data` is expected to be at least `naclac_size()` bytes, valid for
        // reading `Self`. `read_unaligned` avoids alignment requirements in
        // zero-copy instruction-data buffers.
        unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
    }
    #[inline(always)]
    fn naclac_size() -> usize {
        N
    }
}

/// Deserializes an instruction argument from `data` starting at `*offset`,
/// advancing `*offset` past the consumed bytes. Unlike `NaclacPod` (fixed
/// size, known without reading the buffer), this also covers argument types
/// whose encoded length depends on their own content — namely an
/// `#[instruction_args]`-grouped struct containing a `ZcString`/`ZcVec`
/// field, whose macro-generated impl parses its fields one at a time instead
/// of implementing `NaclacPod` (a length-prefixed dynamic field can never be
/// read via a single raw `size_of`-based byte cast). `program.rs`'s
/// instruction dispatch calls this uniformly for every non-collection
/// instruction argument, fixed- or dynamic-size alike.
pub trait NaclacArgs: Sized {
    fn naclac_deserialize(data: &[u8], offset: &mut usize) -> NaclacResult<Self>;
}

impl<T: NaclacPod> NaclacArgs for T {
    #[inline(always)]
    fn naclac_deserialize(data: &[u8], offset: &mut usize) -> NaclacResult<Self> {
        let sz = T::naclac_size();
        if data.len() < *offset + sz {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let val = T::naclac_from_bytes(&data[*offset..*offset + sz]);
        *offset += sz;
        Ok(val)
    }
}

#[cfg(feature = "pinocchio")]
impl core::ops::Deref for Address {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "pinocchio")]
impl core::fmt::Debug for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Address(")?;
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ")")
    }
}

#[cfg(feature = "pinocchio")]
impl Address {
    pub const fn new_from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// For compatibility with Solana-style code expecting .to_bytes()
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Convert from a pinocchio `Address`.
    /// SAFETY: `Address` is `#[repr(transparent)]` over `[u8; 32]`.
    #[inline(always)]
    pub fn from_address(addr: &PinocchioAddress) -> Self {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32].
        // We use a pointer read here to ensure no alignment issues during the cast.
        Self(unsafe { *(addr as *const PinocchioAddress as *const [u8; 32]) })
    }

    /// Borrow as a pinocchio `Address` reference.
    /// SAFETY: `Address` is `#[repr(transparent)]` over `[u8; 32]`.
    #[inline(always)]
    pub fn as_address(&self) -> &PinocchioAddress {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32], representing a 32-byte account address.
        unsafe { &*(self.0.as_ptr() as *const PinocchioAddress) }
    }
}

#[cfg(feature = "pinocchio")]
impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// SYSTEM_PROGRAM_ID is all zeros — system program address
#[cfg(feature = "pinocchio")]
// SAFETY: pinocchio_system::ID is an `Address` (a transparent [u8; 32] wrapper) which is memory-identical to our `Address`.
pub const SYSTEM_PROGRAM_ID: Address = unsafe { core::mem::transmute(pinocchio_system::ID) };

#[cfg(feature = "pinocchio")]
pub use pinocchio_system;

#[cfg(feature = "pinocchio")]
pub struct RefMut<'a> {
    slice: &'a mut [u8],
}

#[cfg(feature = "pinocchio")]
impl<'a> core::ops::Deref for RefMut<'a> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> core::ops::DerefMut for RefMut<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

#[cfg(feature = "pinocchio")]
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct AccountInfo {
    pub view: AccountView,
}

#[cfg(feature = "pinocchio")]
impl core::ops::Deref for AccountInfo {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl AccountInfo {
    pub fn data_is_empty(&self) -> bool {
        self.view.data_len() == 0
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: The AccountView provides a valid data pointer and length from the Solana runtime.
        unsafe { core::slice::from_raw_parts(self.view.data_ptr(), self.view.data_len()) }
    }

    pub fn owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }

    pub fn lamports(&self) -> u64 {
        self.view.lamports()
    }

    pub fn is_signer(&self) -> bool {
        self.view.is_signer()
    }

    pub fn is_writable(&self) -> bool {
        self.view.is_writable()
    }

    pub fn is_executable(&self) -> bool {
        self.view.executable()
    }

    pub fn try_borrow_data(&self) -> Result<&[u8]> {
        Ok(self.data())
    }

    pub fn try_borrow_mut_data(&self) -> Result<RefMut<'_>> {
        let view = self.view;
        // SAFETY: The AccountView provides a valid mutable data pointer and length from the Solana runtime.
        let slice =
            unsafe { core::slice::from_raw_parts_mut(view.data_ptr() as *mut u8, view.data_len()) };
        Ok(RefMut { slice })
    }

    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_sub(amount)
            .ok_or(crate::error::NaclacError::InsufficientFunds.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_add(amount)
            .ok_or(crate::error::NaclacError::ArithmeticOverflow.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    /// Resize the account's data. `AccountView` is `Copy` (it's just a raw
    /// pointer into runtime memory — see `sub_lamports`/`add_lamports`
    /// above for the same copy-then-mutate pattern), so calling the
    /// `&mut self` `Resize::resize` on a local copy still mutates the real
    /// underlying account.
    pub fn resize(&self, new_len: usize) -> Result<()> {
        let mut view = self.view;
        pinocchio::Resize::resize(&mut view, new_len)
            .map_err(|_| crate::error::NaclacError::InvalidRealloc.err(0))
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
pub type NaclacResult<T = (), E = pinocchio::error::ProgramError> = core::result::Result<T, E>;
#[cfg(feature = "pinocchio")]
pub use NaclacResult as Result;

/// Generic instruction-return-data serialization for `#[instruction]` handlers
/// declared `-> Result<T>`. `#[program]`'s dispatcher calls `to_return_data()`
/// on `Ok(value)` and passes the bytes to `set_return_data` automatically —
/// handlers never call `set_return_data` themselves. An `Option<T>` outer
/// wrapper (e.g. a handler that may or may not have anything to return) is
/// detected at macro-expansion time in `naclac-macros`, not via a blanket
/// trait impl here, since a blanket `impl<T: Pod> for T` and a blanket
/// `impl<T: NaclacReturnData> for Option<T>` are rejected by Rust's coherence
/// checker as potentially overlapping.
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
pub trait NaclacReturnData {
    fn to_return_data(&self) -> Vec<u8>;
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: crate::bytemuck::Pod> NaclacReturnData for T {
    fn to_return_data(&self) -> Vec<u8> {
        crate::bytemuck::bytes_of(self).to_vec()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub trait NaclacReturnData {
    fn to_return_data(&self) -> Vec<u8>;
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: crate::borsh::BorshSerialize> NaclacReturnData for T {
    fn to_return_data(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        T::serialize(self, &mut buf).expect("return data serialization failed");
        buf
    }
}

#[cfg(feature = "pinocchio")]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, pinocchio::error::ProgramError> {
    iter.next()
        .ok_or(crate::error::NaclacError::NotEnoughAccountKeys.err(0))
}

#[cfg(not(feature = "pinocchio"))]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, solana_program::program_error::ProgramError> {
    iter.next()
        .ok_or(crate::error::NaclacError::NotEnoughAccountKeys.err(0))
}

#[cfg(feature = "pinocchio")]
#[allow(unused_variables)]
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: sol_log_data is a native Solana SBF syscall. The runtime guarantees it handles pointer boundaries securely.
    unsafe {
        solana_define_syscall::definitions::sol_log_data(data.as_ptr() as *const u8, data.len() as u64);
    }
}

#[cfg(feature = "pinocchio")]
pub fn sol_log_compute_units() {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        solana_define_syscall::definitions::sol_log_compute_units_();
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        // No-op on non-Solana targets
    }
}

// --- OFFICIAL PROGRAM IDS (PINOCCHIO) ---
#[cfg(feature = "pinocchio")]
pub const TOKEN_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    ))
};
#[cfg(feature = "pinocchio")]
pub const TOKEN_2022_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
    ))
};
#[cfg(feature = "pinocchio")]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
    ))
};
#[cfg(feature = "pinocchio")]
pub const RENT_SYSVAR_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "SysvarRent111111111111111111111111111111111"
    ))
};

#[cfg(not(feature = "pinocchio"))]
pub type AccountType = AccountInfo;

#[cfg(feature = "pinocchio")]
pub type AccountType = AccountInfo;

/// Re-export the pinocchio msg! macro into the prelude namespace
/// so `use naclac_lang::prelude::*` makes `msg!` available.
#[cfg(feature = "pinocchio")]
pub use crate::msg;

/// Pinocchio logging — bridges Naclac's `msg!` API to pinocchio_log's Logger.
/// Uses a 128-byte stack-allocated buffer. For longer messages use Logger directly.
#[cfg(feature = "pinocchio")]
#[macro_export]
macro_rules! msg {
    ($msg:expr) => {{
        let mut logger = $crate::pinocchio_log::logger::Logger::<128>::default();
        logger.append($msg);
        logger.log();
    }};
}

/// No-op msg! macro for non-pinocchio, non-debug builds to save space
#[cfg(all(not(feature = "pinocchio"), not(feature = "debug-mode")))]
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {{}};
}

#[cfg(feature = "pinocchio")]
pub use crate::system_program;

#[cfg(feature = "pinocchio")]
pub use crate::base58;

// ===========================================================================
// Shared macros (both backends)
// ===========================================================================

/// Anchor-style event emission macro.
/// Handles both `field: value` and shorthand `field` syntax.
/// Uses mutation pattern so internal padding fields are invisible to users.
#[macro_export]
macro_rules! emit {
    // Main arm: handles mixed shorthand + explicit fields
    ($ty:path { $($field:ident $(: $val:expr)?),* $(,)? }) => {{
        let mut __event = <$ty as core::default::Default>::default();
        $(
            emit!(@__set __event, $field $(, $val)?);
        )*
        __event.emit();
        __event
    }};
    // Internal helper: explicit field: value
    (@__set $ev:ident, $field:ident, $val:expr) => {
        $ev.$field = ($val).into();
    };
    // Internal helper: shorthand field (uses local variable of same name)
    (@__set $ev:ident, $field:ident) => {
        $ev.$field = ($field).into();
    };
    // Pass-through: emit!(my_event_instance)
    ($event:expr) => {{
        let __event = $event;
        __event.emit();
        __event
    }};
}

/// Concise condition checking that returns a ProgramError.
/// Usage: `require!(condition, MyError::SomeVariant)`
#[macro_export]
macro_rules! require {
    ($cond:expr, $err:expr $(,)?) => {
        if !($cond) {
            return Err(($err).into());
        }
    };
}

pub use crate::{address, declare_id, emit, require};

/// Declares the program's static ID constant.
///
/// - Solana backend: delegates to `solana_program::declare_id!`
/// - Pinocchio backend: creates a `pub const ID: Address` using pinocchio's
///   compile-time address parsing.
#[macro_export]
macro_rules! declare_id {
    ($id:expr) => {
        #[cfg(not(feature = "pinocchio"))]
        $crate::solana_program::declare_id!($id);

        #[cfg(feature = "pinocchio")]
        pub const ID: $crate::prelude::Address =
            unsafe { core::mem::transmute($crate::pinocchio::address::address!($id)) };
        #[cfg(feature = "pinocchio")]
        pub fn id() -> $crate::prelude::Address {
            ID
        }
    };
}

/// Parses a base58 address literal into a `$crate::prelude::Address` constant,
/// usable anywhere (not just a crate's own program ID, unlike `declare_id!`).
/// Resolves the same Pinocchio-vs-Solana backend split `declare_id!` does
/// internally, so callers never need to know `pinocchio::address::address!`
/// returns a different (structurally identical) `Address` type that needs an
/// explicit conversion.
#[macro_export]
macro_rules! address {
    ($id:expr) => {{
        #[cfg(not(feature = "pinocchio"))]
        {
            $crate::solana_address::address!($id)
        }
        #[cfg(feature = "pinocchio")]
        {
            let __addr: $crate::prelude::Address =
                unsafe { core::mem::transmute($crate::pinocchio::address::address!($id)) };
            __addr
        }
    }};
}

// --- Zero-Copy Helper Types ---

/// A zero-copy compatible boolean wrapper.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Bool(pub u8);

impl From<u8> for Bool {
    fn from(b: u8) -> Self {
        Self(b)
    }
}

impl From<bool> for Bool {
    fn from(b: bool) -> Self {
        Self(if b { 1 } else { 0 })
    }
}

impl From<Bool> for bool {
    fn from(b: Bool) -> Self {
        b.0 != 0
    }
}

/// A zero-copy compatible Option wrapper.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opt<T: bytemuck::Pod> {
    pub value: T,
    pub has_value: u8,
    pub _padding: [u8; 7], // Ensure alignment for most types (u64, etc)
}

unsafe impl<T: bytemuck::Pod> bytemuck::Zeroable for Opt<T> {}
unsafe impl<T: bytemuck::Pod> bytemuck::Pod for Opt<T> {}

impl<T: bytemuck::Pod> Opt<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            has_value: 1,
            _padding: [0; 7],
        }
    }
    pub fn none(default: T) -> Self {
        Self {
            value: default,
            has_value: 0,
            _padding: [0; 7],
        }
    }
}

impl<T: bytemuck::Pod> From<Option<T>> for Opt<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::new(v),
            None => Self {
                value: unsafe { core::mem::zeroed() },
                has_value: 0,
                _padding: [0; 7],
            },
        }
    }
}
