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
    use crate::prelude::{vec, CpiHandle, CpiHandleMut, Result};

    /// The Solana System Program ID (all zeros)
    pub const ID: solana_address::Address = solana_address::Address::new_from_array([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]);

    pub fn transfer(from: CpiHandleMut<'_>, to: CpiHandleMut<'_>, lamports: u64) -> Result<()> {
        transfer_signed(from, to, lamports, &[])
    }

    /// Builds the `Transfer` instruction by hand (`Instruction::new_with_bytes`)
    /// rather than via `solana_system_interface::instruction::transfer`, whose
    /// `new_with_wincode`/`new_with_bincode` path pulls generic-serializer and
    /// `Debug`-formatting machinery into the program for four fixed fields.
    /// Discriminant (2) and account-meta shape verified against the real
    /// `solana_system_interface::instruction::{SystemInstruction, transfer}`
    /// source, not guessed.
    pub fn transfer_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let accounts = vec![
            solana_instruction::AccountMeta::new(*from.info.key, true),
            solana_instruction::AccountMeta::new(*to.info.key, false),
        ];

        let mut ix_data = [0u8; 12];
        ix_data[0..4].copy_from_slice(&2u32.to_le_bytes()); // Transfer discriminator = 2
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());

        let ix = solana_instruction::Instruction::new_with_bytes(ID, &ix_data, accounts);

        let accounts = [CpiHandle::from(from), CpiHandle::from(to)];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    use crate::cpi::stable::{invoke_signed_raw, StableAccountMeta, StableInstruction};

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
                is_signer: true,
                is_writable: true,
            },
            StableAccountMeta {
                pubkey: to.info.address(),
                is_signer: true,
                is_writable: true,
            },
        ];

        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());

        let stable_ix = StableInstruction::new(ID, &stable_accounts, &ix_data);

        let account_infos = unsafe {
            [
                from.info.to_lifetime(),
                to.info.to_lifetime(),
                program.info.to_lifetime(),
            ]
        };

        invoke_signed_raw(&stable_ix, &account_infos, signer_seeds)
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

    /// Checked, prefund-aware account creation. When `to` holds no lamports
    /// this is the System Program's `CreateAccount` (discriminant 0, accounts
    /// `[from, to]`, funding `lamports`); otherwise `CreateAccountAllowPrefund`
    /// (SIMD-0312: discriminant 13, accounts `[to, from]`, funding the
    /// shortfall `lamports - to.lamports`, saturating at 0). Both are
    /// `[discriminant: u32, amount: u64, space: u64, owner: [u8; 32]]`, with
    /// both accounts writable signers, and must stay byte-identical to
    /// `solana_system_interface::instruction::SystemInstruction`. Fails with
    /// `AccountBorrowFailed` if either account's lamports or data is borrowed
    /// elsewhere, as `solana_cpi::invoke_signed` would.
    pub fn create_account_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let current = **to
            .info
            .lamports
            .try_borrow()
            .map_err(|_| crate::prelude::ProgramError::AccountBorrowFailed)?;
        let (discriminant, amount, first, second) = if current == 0 {
            (0u32, lamports, &from.info, &to.info)
        } else {
            (
                13u32,
                lamports.saturating_sub(current),
                &to.info,
                &from.info,
            )
        };

        for info in [first, second] {
            drop(
                info.lamports
                    .try_borrow_mut()
                    .map_err(|_| crate::prelude::ProgramError::AccountBorrowFailed)?,
            );
            drop(
                info.data
                    .try_borrow_mut()
                    .map_err(|_| crate::prelude::ProgramError::AccountBorrowFailed)?,
            );
        }

        let accounts = [
            StableAccountMeta {
                pubkey: first.address(),
                is_signer: true,
                is_writable: true,
            },
            StableAccountMeta {
                pubkey: second.address(),
                is_signer: true,
                is_writable: true,
            },
        ];
        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&discriminant.to_le_bytes());
        ix_data[4..12].copy_from_slice(&amount.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());
        let instruction = StableInstruction::new(ID, &accounts, &ix_data);

        // SAFETY: these are bitwise aliases of the accounts `first` and
        // `second` point to, which outlive this call, read as the SDK's
        // `AccountInfo` that `AccountInfoData` mirrors (the same layout
        // `to_lifetime` transmutes between). They are never dropped
        // (`ManuallyDrop`), so the `Rc` counts are neither incremented nor
        // decremented, and they do not escape this function.
        let infos = core::mem::ManuallyDrop::new(unsafe {
            [
                core::ptr::read(first.data_ref() as *const crate::prelude::AccountInfoData
                    as *const solana_account_info::AccountInfo<'static>),
                core::ptr::read(second.data_ref() as *const crate::prelude::AccountInfoData
                    as *const solana_account_info::AccountInfo<'static>),
            ]
        });
        invoke_signed_raw(&instruction, &*infos, signer_seeds)
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

    /// Checked, prefund-aware account creation. When `to` holds no lamports
    /// this is the System Program's `CreateAccount` (discriminant 0,
    /// accounts `[from, to]`, funding `lamports`); otherwise it is
    /// `CreateAccountAllowPrefund` (discriminant 13, accounts `[to]` plus
    /// `from` only when `to` is short of `lamports`, funding the shortfall).
    /// Both share the 52-byte data layout `[discriminant: u32, amount: u64,
    /// space: u64, owner: [u8; 32]]` and must stay byte-identical to
    /// `pinocchio_system::instructions::{CreateAccount,
    /// CreateAccountAllowPrefund}`. Fails with `AccountBorrowFailed` if any
    /// account passed to the CPI has its data borrowed.
    #[inline(always)]
    fn create_account_checked_raw(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        let (from, to) = (*from, *to);
        let current = to.lamports();
        let (discriminant, amount, first, second, account_count): (u32, u64, _, _, usize) =
            if current == 0 {
                if from.is_borrowed() | to.is_borrowed() {
                    return Err(crate::prelude::ProgramError::AccountBorrowFailed);
                }
                (0, lamports, from, to, 2)
            } else {
                let shortfall = lamports.saturating_sub(current);
                if shortfall == 0 {
                    if to.is_borrowed() {
                        return Err(crate::prelude::ProgramError::AccountBorrowFailed);
                    }
                    (13, 0, to, from, 1)
                } else {
                    if to.is_borrowed() | from.is_borrowed() {
                        return Err(crate::prelude::ProgramError::AccountBorrowFailed);
                    }
                    (13, shortfall, to, from, 2)
                }
            };

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(first.address()),
            ::pinocchio::instruction::InstructionAccount::writable_signer(second.address()),
        ];
        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&discriminant.to_le_bytes());
        ix_data[4..12].copy_from_slice(&amount.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());
        let cpi_accounts = [
            ::pinocchio::cpi::CpiAccount::from(&first),
            ::pinocchio::cpi::CpiAccount::from(&second),
        ];

        // SAFETY: `account_count` is 1 or 2, within both arrays; the borrow
        // check above covers every account included in the call.
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(
                &::pinocchio::instruction::InstructionView {
                    program_id: &ID,
                    accounts: core::slice::from_raw_parts(ix_accounts.as_ptr(), account_count),
                    data: &ix_data,
                },
                core::slice::from_raw_parts(cpi_accounts.as_ptr(), account_count),
                signer_seeds,
            );
        }

        Ok(())
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
#[cfg(feature = "pinocchio")]
pub use pinocchio_system::{
    create_account, create_account_checked_for_init, create_account_signed,
    create_account_signed_unchecked, create_account_unchecked, create_account_unchecked_for_init,
    transfer, transfer_signed, ID,
};
#[cfg(not(feature = "pinocchio"))]
pub use solana_system::{
    create_account, create_account_signed, create_account_signed_unchecked,
    create_account_unchecked, transfer, transfer_signed, ID,
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
