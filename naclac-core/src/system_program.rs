// ===========================================================================
// system_program.rs — Dual-backend system program helpers
// ===========================================================================

//! # System Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking native Solana System Program instructions.
//! Enforces borrow-checking via CPI handles.
//!
//! `create_account`/`create_account_signed` always resolve to the *checked*
//! path: borrow-checked on pinocchio (via the real `pinocchio_system`
//! instruction structs, which verify no account is already borrowed before
//! invoking) and, on both backends, aware that the target account may
//! already hold lamports (e.g. a PDA pre-funded by a plain SOL transfer
//! before this instruction ran) — falling back to `CreateAccountAllowPrefund`
//! (SIMD-0312) instead of failing with `AccountAlreadyInUse`, the way plain
//! `CreateAccount` would. `create_account_unchecked`/
//! `create_account_signed_unchecked` are the raw, no-prefund-awareness
//! primitives — on pinocchio, they also skip the runtime `is_borrowed()`
//! aliasing check via `unsafe { invoke_signed_unchecked(...) }` — for
//! callers who know what they're doing and want to skip that overhead.
//! `#[derive(Accounts)]`'s `init`/`init_if_needed` codegen always calls the
//! checked path; `init_unchecked` is the one exception, calling this
//! unchecked path instead (and is rejected at compile time on an ATA field,
//! which has no create-account call of its own to redirect).

use crate::prelude::{ToCpiHandle, ToCpiHandleMut};

// SOLANA BACKEND
#[cfg(not(feature = "pinocchio"))]
mod solana_system {
    use crate::prelude::{CpiHandle, CpiHandleMut, Result};

    /// The Solana System Program ID (all zeros)
    pub const ID: solana_address::Address = solana_address::Address::new_from_array([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]);

    pub fn transfer(from: CpiHandleMut<'_>, to: CpiHandleMut<'_>, lamports: u64) -> Result<()> {
        transfer_signed(from, to, lamports, &[])
    }

    pub fn transfer_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let ix =
            solana_system_interface::instruction::transfer(from.info.key, to.info.key, lamports);
        let accounts = [CpiHandle::from(from), CpiHandle::from(to)];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[repr(C)]
    struct StableAccountMeta {
        pub pubkey: solana_address::Address,
        pub is_writable: bool,
        pub is_signer: bool,
    }

    #[repr(C)]
    struct StableVec<T> {
        pub ptr: *const T,
        pub len: usize,
        pub cap: usize,
    }

    #[repr(C)]
    struct StableInstruction {
        pub accounts: StableVec<StableAccountMeta>,
        pub data: StableVec<u8>,
        pub program_id: solana_address::Address,
    }

    extern "C" {
        fn sol_invoke_signed_rust(
            instruction_addr: *const u8,
            account_infos_addr: *const u8,
            account_infos_len: u64,
            signers_seeds_addr: *const u8,
            signers_seeds_len: u64,
        ) -> u64;
    }

    pub fn create_account_unchecked(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
    ) -> Result<()> {
        create_account_signed_unchecked(from, to, program, lamports, space, owner, &[])
    }

    /// Raw, no-prefund-awareness `CreateAccount` CPI via the low-level FFI
    /// `sol_invoke_signed_rust` entry point rather than `solana_program`'s
    /// own `invoke_signed` — the pre-existing implementation this module
    /// has always used. Fails with `AccountAlreadyInUse` if `to` already
    /// holds lamports; see `create_account_signed` for the checked,
    /// prefund-aware equivalent.
    pub fn create_account_signed_unchecked(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        // The instruction below always targets the real, hardcoded `ID`
        // regardless of `program` — but the low-level CPI mechanism still
        // needs `program`'s `AccountInfo` present to locate/invoke it, so a
        // caller passing the wrong account here would otherwise fail with
        // an opaque "account not found"-style runtime error instead of a
        // clear one.
        if program.info.address() != ID {
            return Err(crate::error::NaclacError::ProgramIdMismatch.into());
        }

        let stable_accounts = [
            StableAccountMeta {
                pubkey: from.info.address(),
                is_writable: true,
                is_signer: true,
            },
            StableAccountMeta {
                pubkey: to.info.address(),
                is_writable: true,
                is_signer: true,
            },
        ];

        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());

        let stable_ix = StableInstruction {
            accounts: StableVec {
                ptr: stable_accounts.as_ptr(),
                len: stable_accounts.len(),
                cap: stable_accounts.len(),
            },
            data: StableVec {
                ptr: ix_data.as_ptr(),
                len: ix_data.len(),
                cap: ix_data.len(),
            },
            program_id: ID,
        };

        let account_infos = unsafe {
            [
                from.info.to_lifetime(),
                to.info.to_lifetime(),
                program.info.to_lifetime(),
            ]
        };

        unsafe {
            let result = sol_invoke_signed_rust(
                &stable_ix as *const StableInstruction as *const u8,
                account_infos.as_ptr() as *const u8,
                account_infos.len() as u64,
                signer_seeds.as_ptr() as *const u8,
                signer_seeds.len() as u64,
            );
            if result != 0 {
                return Err(solana_program::program_error::ProgramError::Custom(
                    result as u32,
                ));
            }
        }
        Ok(())
    }

    /// Fallback for a target account that already holds lamports (e.g. a
    /// PDA that received a plain SOL transfer before this instruction ran)
    /// — plain `CreateAccount` requires a zero-lamport target and fails
    /// with `AccountAlreadyInUse` otherwise. Uses the real
    /// `solana_system_interface::instruction::create_account_allow_prefund`
    /// (SIMD-0312) rather than a hand-rolled `Allocate`+`Assign` sequence,
    /// since the real instruction already handles crediting the shortfall
    /// itself in one CPI.
    fn create_account_allow_prefund_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let current = to.info.lamports();
        let ix = solana_system_interface::instruction::create_account_allow_prefund(
            &to.info.address(),
            Some((&from.info.address(), lamports.saturating_sub(current))),
            space,
            owner,
        );
        // SAFETY: lifetime-erased for the duration of this CPI only, same
        // as every other call site in this module.
        let account_infos = unsafe { [to.info.to_lifetime(), from.info.to_lifetime()] };
        solana_program::program::invoke_signed(&ix, &account_infos, signer_seeds)
    }

    /// Checked, prefund-aware `CreateAccount` — the real
    /// `solana_system_interface::instruction::create_account` builder plus
    /// the standard `solana_program::program::invoke_signed` (which enforces
    /// the normal `AccountInfo` borrow rules), instead of this module's own
    /// lower-level FFI path. Used only when `to` is confirmed empty; see
    /// `create_account_allow_prefund_signed` for the pre-funded case.
    fn create_account_checked_empty_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let ix = solana_system_interface::instruction::create_account(
            &from.info.address(),
            &to.info.address(),
            lamports,
            space,
            owner,
        );
        let account_infos = unsafe { [from.info.to_lifetime(), to.info.to_lifetime()] };
        solana_program::program::invoke_signed(&ix, &account_infos, signer_seeds)
    }

    pub fn create_account(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
    ) -> Result<()> {
        create_account_signed(from, to, program, lamports, space, owner, &[])
    }

    pub fn create_account_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        if to.info.lamports() > 0 {
            create_account_allow_prefund_signed(from, to, lamports, space, owner, signer_seeds)
        } else {
            create_account_checked_empty_signed(from, to, lamports, space, owner, signer_seeds)
        }
    }
}

// PINOCCHIO BACKEND
#[cfg(feature = "pinocchio")]
mod pinocchio_system {
    use crate::prelude::{CpiHandle, CpiHandleMut, Result};

    // Re-use the system program ID directly from pinocchio-system crate
    pub use ::pinocchio_system::ID;

    pub fn transfer(from: CpiHandleMut<'_>, to: CpiHandleMut<'_>, lamports: u64) -> Result<()> {
        transfer_signed(from, to, lamports, &[])
    }

    pub fn transfer_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let from_address = *from.info.view.address();
        let to_address = *to.info.view.address();
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(&from_address),
            ::pinocchio::instruction::InstructionAccount::writable(&to_address),
        ];
        let mut ix_data = [0u8; 12];
        ix_data[0] = 2; // system transfer discriminant
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: &ID,
            accounts: &ix_accounts,
            data: &ix_data,
        };

        let cpi_accounts = [CpiHandle::from(from), CpiHandle::from(to)];

        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &cpi_accounts, signer_seeds)
    }

    /// Expands to a `let #signers_var: &[::pinocchio::cpi::Signer] = ...;`
    /// binding, converting naclac's backend-uniform `&[&[&[u8]]]` signer
    /// seeds (`$seeds`) into pinocchio's own `Signer`/`Seed` types without a
    /// heap allocation. A macro rather than a function: each `Signer` built
    /// here borrows from the seed buffer built in the same scope, and
    /// threading that self-reference across a function boundary risks a
    /// subtly wrong lifetime signature — expanding inline keeps the exact,
    /// already-correct borrow shape at every call site. Shared by every
    /// pinocchio `_unchecked`/checked `CreateAccount`-family function below
    /// instead of duplicating this conversion in each one.
    macro_rules! pinocchio_signers_from_seeds {
        ($seeds:expr, $signers_var:ident) => {
            if $seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
                return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
            }
            let mut __signers: [::pinocchio::cpi::Signer; crate::prelude::MAX_CPI_SIGNERS] =
                unsafe { core::mem::zeroed() };
            let mut __seeds_buffer: [::pinocchio::cpi::Seed;
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER * crate::prelude::MAX_CPI_SIGNERS] =
                unsafe { core::mem::zeroed() };
            let mut __seed_ranges = [(0usize, 0usize); crate::prelude::MAX_CPI_SIGNERS];

            let mut __seed_idx = 0;

            for (i, seed_parts) in $seeds.iter().enumerate() {
                if seed_parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                    return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
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

    /// Raw, no-borrow-check `CreateAccount` — the pre-existing hand-rolled
    /// `invoke_signed_unchecked` implementation this module has always
    /// used. Fails with `AccountAlreadyInUse` if `to` already holds
    /// lamports; see `create_account_checked_raw` for the checked,
    /// prefund-aware equivalent.
    #[inline(always)]
    fn create_account_unchecked_raw(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(from.address()),
            ::pinocchio::instruction::InstructionAccount::writable_signer(to.address()),
        ];
        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // system create_account discriminant
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: &ID,
            accounts: &ix_accounts,
            data: &ix_data,
        };

        let cpi_accounts = [
            ::pinocchio::cpi::CpiAccount::from(from),
            ::pinocchio::cpi::CpiAccount::from(to),
        ];

        // SAFETY: Bypasses Pinocchio's runtime check, safe as caller validates borrows
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(&instruction, &cpi_accounts, signer_seeds);
        }

        Ok(())
    }

    /// Checked, prefund-aware `CreateAccount`/`CreateAccountAllowPrefund` —
    /// goes through the real `pinocchio_system::instructions::{CreateAccount,
    /// CreateAccountAllowPrefund}` structs directly, both of which perform
    /// their own `is_borrowed()` check on every account before invoking
    /// (unlike `create_account_unchecked_raw` above, naclac's own hand-rolled
    /// `invoke_signed_unchecked` call with no check of its own). Routes to
    /// the prefund variant when `to` already holds lamports (e.g. a PDA
    /// pre-funded by a plain SOL transfer before this instruction ran) —
    /// plain `CreateAccount` requires a zero-lamport target and fails
    /// otherwise.
    #[inline(always)]
    fn create_account_checked_raw(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        let current = to.lamports();
        if current > 0 {
            // `Funding.lamports` is the amount to *transfer*, i.e. the
            // shortfall against what the account already holds, not the
            // full target `lamports`.
            let shortfall = lamports.saturating_sub(current);
            ::pinocchio_system::instructions::CreateAccountAllowPrefund {
                to,
                space,
                // The real struct wants pinocchio's own `Address` type, not
                // naclac's — `as_address()` is the established,
                // verified-safe bridge between the two (same type at the
                // byte level, used the same way elsewhere in this
                // codebase, e.g. `naclac-token/src/extensions/mod.rs`'s
                // `ix_addr`).
                owner: owner.as_address(),
                funding: if shortfall == 0 {
                    None
                } else {
                    Some(::pinocchio_system::instructions::Funding {
                        from,
                        lamports: shortfall,
                    })
                },
            }
            .invoke_signed(signer_seeds)
        } else {
            ::pinocchio_system::instructions::CreateAccount {
                from,
                to,
                lamports,
                space,
                owner: owner.as_address(),
            }
            .invoke_signed(signer_seeds)
        }
    }

    pub fn create_account_unchecked(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
    ) -> Result<()> {
        create_account_signed_unchecked(from, to, _program, lamports, space, owner, &[])
    }

    pub fn create_account_signed_unchecked(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        pinocchio_signers_from_seeds!(signer_seeds, signers);
        create_account_unchecked_raw(
            &from.info.view,
            &to.info.view,
            lamports,
            space,
            owner,
            signers,
        )
    }

    pub fn create_account(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
    ) -> Result<()> {
        create_account_signed(from, to, program, lamports, space, owner, &[])
    }

    pub fn create_account_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        pinocchio_signers_from_seeds!(signer_seeds, signers);
        create_account_checked_raw(
            &from.info.view,
            &to.info.view,
            lamports,
            space,
            owner,
            signers,
        )
    }

    /// Direct entry point for `naclac-macros`' `init`/`init_if_needed`
    /// codegen on the pinocchio backend — takes already-built
    /// `AccountView`s and already-converted `pinocchio::cpi::Signer`s
    /// (both already in scope in the generated code) rather than
    /// `CpiHandleMut`/raw seeds, avoiding a redundant round-trip through
    /// `CpiHandleMut` and `pinocchio_signers_from_seeds!` for state the
    /// caller already has. Always the checked, prefund-aware path — use
    /// `create_account_unchecked_for_init` for `init_unchecked`.
    pub fn create_account_checked_for_init(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        create_account_checked_raw(from, to, lamports, space, owner, signer_seeds)
    }

    /// Same already-built-view calling convention as
    /// `create_account_checked_for_init`, but the unchecked path —
    /// `naclac-macros`' `init_unchecked` codegen entry point. Skips both the
    /// prefund-awareness branch and pinocchio's runtime `is_borrowed()`
    /// aliasing check (see `create_account_unchecked_raw`'s own
    /// `unsafe { invoke_signed_unchecked(...) }`) — safe to call only when
    /// the caller has not retained a live borrow on `from`'s or `to`'s
    /// account data across this call.
    pub fn create_account_unchecked_for_init(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        create_account_unchecked_raw(from, to, lamports, space, owner, signer_seeds)
    }
}

// ===========================================================================
// Re-exports
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_system::{
    create_account, create_account_signed, create_account_signed_unchecked,
    create_account_unchecked, transfer, transfer_signed, ID,
};

#[cfg(feature = "pinocchio")]
pub use pinocchio_system::{
    create_account, create_account_checked_for_init, create_account_signed,
    create_account_signed_unchecked, create_account_unchecked, create_account_unchecked_for_init,
    transfer, transfer_signed, ID,
};

#[derive(Debug)]
pub struct SystemTransferAccounts<'a, A, B> {
    pub from: &'a mut A,
    pub to: &'a mut B,
}

#[derive(Debug)]
pub struct CreateAccountAccounts<'a, A, B> {
    pub from: &'a mut A,
    pub to: &'a mut B,
}

impl crate::wrappers::Program<crate::wrappers::System> {
    pub fn transfer<'a, A, B>(
        &self,
        accounts: SystemTransferAccounts<'a, A, B>,
        amount: u64,
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        transfer(from_h, to_h, amount)
    }

    pub fn transfer_signed<'a, A, B>(
        &self,
        accounts: SystemTransferAccounts<'a, A, B>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        transfer_signed(from_h, to_h, amount, signer_seeds)
    }

    pub fn create_account<'a, A, B>(
        &self,
        accounts: CreateAccountAccounts<'a, A, B>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        let program_h = self.to_cpi_handle();
        create_account(from_h, to_h, program_h, lamports, space, owner)
    }

    pub fn create_account_signed<'a, A, B>(
        &self,
        accounts: CreateAccountAccounts<'a, A, B>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        let program_h = self.to_cpi_handle();
        create_account_signed(
            from_h,
            to_h,
            program_h,
            lamports,
            space,
            owner,
            signer_seeds,
        )
    }
}
