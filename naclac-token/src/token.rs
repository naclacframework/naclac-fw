// ===========================================================================
// token.rs — SPL Token & Token-2022 CPI helpers
// ===========================================================================

//! # SPL Token & Token-2022 CPI Abstractions
//!
//! Provides a unified interface for interacting with SPL Token programs across both
//! standard Solana and Pinocchio, using borrow-checked CPI handles.

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Discriminator, NaclacError, Result};
use crate::wrappers::{
    Interface, Program, ToAddress, ToCpiHandle, ToCpiHandleMut, Token, Token2022, TokenInterface,
};

#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
use spl_token;
#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
use spl_token_2022;

// PINOCCHIO DEPS
#[cfg(feature = "pinocchio")]
use pinocchio::cpi::{Seed, Signer};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum AuthorityType {
    MintTokens,
    FreezeAccount,
    AccountOwner,
    CloseAccount,
}

// ===========================================================================
// CPI HELPER FUNCTIONS
// ===========================================================================

/// Numeric parameters for a checked token transfer.
/// Grouping amount + decimals keeps `transfer_checked_signed` under clippy's
/// argument-count limit while remaining fully explicit at call sites.
#[derive(Copy, Clone, Debug)]
pub struct CheckedTransferParams {
    pub amount: u64,
    pub decimals: u8,
}

// --- TRANSFER CHECKED ---
pub fn transfer_checked(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    transfer_checked_signed(
        program,
        from,
        mint,
        to,
        authority,
        CheckedTransferParams { amount, decimals },
        &[],
    )
}

pub fn transfer_checked_signed(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    params: CheckedTransferParams,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let CheckedTransferParams { amount, decimals } = params;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::transfer_checked(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
                decimals,
            )?
        } else {
            spl_token::instruction::transfer_checked(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
                decimals,
            )?
        };
        let accounts = [
            CpiHandle::from(from),
            mint,
            CpiHandle::from(to),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::TransferChecked {
            token_program: program.info.view.address(),
            from: &from.info.view,
            mint: &mint.info.view,
            to: &to.info.view,
            authority: &authority.info.view,
            amount,
            decimals,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- MINT TO ---
pub fn mint_to(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    mint_to_signed(program, mint, to, authority, amount, &[])
}

pub fn mint_to_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::mint_to(
                &program.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::mint_to(
                &program.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [
            CpiHandle::from(mint),
            CpiHandle::from(to),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::MintTo {
            token_program: program.info.view.address(),
            mint: &mint.info.view,
            account: &to.info.view,
            mint_authority: &authority.info.view,
            amount,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- BURN ---
pub fn burn(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    from: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    burn_signed(program, mint, from, authority, amount, &[])
}

pub fn burn_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    from: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::burn(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::burn(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [
            CpiHandle::from(from),
            CpiHandle::from(mint),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::Burn {
            token_program: program.info.view.address(),
            account: &from.info.view,
            mint: &mint.info.view,
            authority: &authority.info.view,
            amount,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- CLOSE ACCOUNT ---
pub fn close_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    close_account_signed(program, account, destination, authority, &[])
}

pub fn close_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::close_account(
                &program.address(),
                &account.info.address(),
                &destination.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::close_account(
                &program.address(),
                &account.info.address(),
                &destination.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [
            CpiHandle::from(account),
            CpiHandle::from(destination),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::CloseAccount {
            token_program: program.info.view.address(),
            account: &account.info.view,
            destination: &destination.info.view,
            authority: &authority.info.view,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- SYNC NATIVE ---
// Recomputes a native (WSOL) token account's reported `amount` from its
// actual lamport balance — the second half of the standard "wrap SOL"
// pattern (transfer lamports into the account, then call this). Genuinely
// permissionless: the instruction takes only the token account itself, no
// authority to sign for, so unlike every other helper in this file there is
// no `_signed` variant — one would have nothing to sign.
pub fn sync_native(program: CpiHandle<'_>, native_token: CpiHandleMut<'_>) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::sync_native(
                &program.address(),
                &native_token.info.address(),
            )?
        } else {
            spl_token::instruction::sync_native(&program.address(), &native_token.info.address())?
        };
        let accounts = [CpiHandle::from(native_token), program];
        crate::cpi::invoke_signed(&ix, &accounts, &[])
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::SyncNative {
            token_program: program.info.view.address(),
            native_token: &native_token.info.view,
        };
        ix.invoke()?;
        Ok(())
    }
}

/// Maximum accounts `sync_native_with_extra_accounts` can pad onto the real
/// `SyncNative` CPI beyond `native_token` and `rent_sysvar`.
pub const MAX_SYNC_NATIVE_EXTRA_ACCOUNTS: usize = 4;

/// Same as `sync_native`, but appends `extra_accounts` to the CPI's account
/// list as harmless, functionally-unused pass-through entries, after a
/// genuine `rent_sysvar` account in slot 1.
///
/// `rent_sysvar` isn't optional here even though plain `sync_native` never
/// needs one: the real classic-Token `SyncNative` processor's second slot
/// (confirmed via `pinocchio-token`'s own `SyncNative` doc comment and via
/// `reference/fee-tier-probe/src/bin/probe31.rs`'s decoded real transaction)
/// is specifically reserved for the Rent sysvar, not an arbitrary extra —
/// passing a padding account there instead produces a real `InvalidArgument`
/// error from the on-chain program, empirically confirmed while building
/// `pump::migrate`. `extra_accounts` start at slot 2.
///
/// The padding itself is needed whenever a preceding raw
/// (`sub_lamports`/`add_lamports`) lamport mutation touched `native_token`
/// — or an account whose balance must be reconciled alongside it, such as
/// the source `native_token`'s lamports were debited from — since neither
/// of those raw writes go through Solana's tracked lamport-accounting path.
/// The runtime only reconciles a raw lamport write into its own tracked
/// bookkeeping for accounts present in whatever CPI is entered next; an
/// account debited or credited raw and never passed to any subsequent CPI
/// is invisible to that reconciliation, so if its counterpart *is* passed to
/// a CPI (as `native_token` here always is), the runtime sees only one side
/// of the change and rejects the whole instruction as unbalanced. Passing
/// every other account touched by the same raw mutation as `extra_accounts`
/// here — the standard Solana "remaining accounts" idiom, just threaded
/// through an inner CPI instead of the top-level instruction — makes the
/// runtime reconcile the full, genuinely-balanced set together instead of
/// one side in isolation.
pub fn sync_native_with_extra_accounts(
    program: CpiHandle<'_>,
    native_token: CpiHandleMut<'_>,
    rent_sysvar: CpiHandle<'_>,
    extra_accounts: &[CpiHandle<'_>],
) -> Result<()> {
    if extra_accounts.len() > MAX_SYNC_NATIVE_EXTRA_ACCOUNTS {
        return Err(NaclacError::TooManyExtraAccounts.into());
    }

    #[cfg(not(feature = "pinocchio"))]
    {
        let mut ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::sync_native(
                &program.address(),
                &native_token.info.address(),
            )?
        } else {
            spl_token::instruction::sync_native(&program.address(), &native_token.info.address())?
        };
        ix.accounts.insert(
            1,
            solana_program::instruction::AccountMeta::new_readonly(rent_sysvar.address(), false),
        );
        ix.accounts.extend(
            extra_accounts
                .iter()
                .map(|extra| solana_program::instruction::AccountMeta::new(extra.address(), false)),
        );

        let native_handle: CpiHandle<'_> = CpiHandle::from(native_token);
        let mut accounts: [CpiHandle<'_>; MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 3] =
            core::array::from_fn(|_| native_handle.clone());
        accounts[1] = rent_sysvar;
        accounts[2] = program;
        let extra_len = extra_accounts.len();
        accounts[3..3 + extra_len].clone_from_slice(extra_accounts);

        crate::cpi::invoke_signed(&ix, &accounts[..3 + extra_len], &[])
    }

    #[cfg(feature = "pinocchio")]
    {
        let extra_len = extra_accounts.len();
        let mut addresses = [*native_token.info.view.address(); MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 2];
        addresses[1] = *rent_sysvar.info.view.address();
        for (slot, extra) in addresses[2..].iter_mut().zip(extra_accounts.iter()) {
            *slot = *extra.info.view.address();
        }

        let ix_accounts_all: [::pinocchio::instruction::InstructionAccount;
            MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 2] = core::array::from_fn(|i| match i {
            0 => ::pinocchio::instruction::InstructionAccount::writable(&addresses[0]),
            1 => ::pinocchio::instruction::InstructionAccount::readonly(&addresses[1]),
            _ => ::pinocchio::instruction::InstructionAccount::writable(&addresses[i]),
        });
        let ix_accounts = &ix_accounts_all[..extra_len + 2];

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: ix_accounts,
            data: &[17],
        };

        let native_handle: CpiHandle<'_> = native_token.into();
        let mut cpi_handles = [native_handle; MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 2];
        cpi_handles[1] = rent_sysvar;
        cpi_handles[2..2 + extra_len].clone_from_slice(extra_accounts);

        crate::cpi::invoke_signed_pinocchio_handles(
            &instruction,
            &cpi_handles[..extra_len + 2],
            &[],
        )
    }
}

// --- APPROVE ---
pub fn approve(
    program: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    delegate: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    approve_signed(program, to, delegate, authority, amount, &[])
}

pub fn approve_signed(
    program: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    delegate: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::approve(
                &program.address(),
                &to.info.address(),
                &delegate.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::approve(
                &program.address(),
                &to.info.address(),
                &delegate.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [CpiHandle::from(to), delegate, authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::Approve {
            token_program: program.info.view.address(),
            source: &to.info.view,
            delegate: &delegate.info.view,
            authority: &authority.info.view,
            amount,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- REVOKE ---
pub fn revoke(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    revoke_signed(program, source, authority, &[])
}

pub fn revoke_signed(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::revoke(
                &program.address(),
                &source.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::revoke(
                &program.address(),
                &source.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(source), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::Revoke {
            token_program: program.info.view.address(),
            source: &source.info.view,
            authority: &authority.info.view,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            ix.invoke()?;
        } else {
            ix.invoke_signed(signers)?;
        }
        Ok(())
    }
}

// --- INITIALIZE MINT ---
pub fn initialize_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<()> {
    initialize_mint_signed(
        program,
        mint,
        decimals,
        mint_authority,
        freeze_authority,
        &[],
    )
}

pub fn initialize_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![20u8]; // InitializeMint2
        data.push(decimals);
        data.extend_from_slice(mint_authority.as_ref());
        if let Some(freeze) = freeze_authority {
            data.push(1);
            data.extend_from_slice(freeze.as_ref());
        } else {
            data.push(0);
            data.extend_from_slice(&[0u8; 32]);
        }

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
                mint.info.address(),
                false,
            )],
            data,
        };
        let accounts = [CpiHandle::from(mint), program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::InitializeMint2 {
            token_program: program.info.view.address(),
            mint: &mint.info.view,
            decimals,
            mint_authority: mint_authority.as_address(),
            freeze_authority: freeze_authority.map(|a| a.as_address()),
        };

        ix.invoke()
    }
}

// --- INITIALIZE ACCOUNT ---
pub fn initialize_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    initialize_account_signed(program, account, mint, authority, &[])
}

pub fn initialize_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = [0u8; 33];
        data[0] = 18; // InitializeAccount3 discriminant
        data[1..].copy_from_slice(authority.info.address().as_ref());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(account.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(mint.info.address(), false),
            ],
            data: data.to_vec(),
        };
        let accounts = [CpiHandle::from(account), mint, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::InitializeAccount3 {
            token_program: program.info.view.address(),
            account: &account.info.view,
            mint: &mint.info.view,
            owner: authority.info.view.address(),
        };

        ix.invoke()
    }
}

// --- FREEZE ACCOUNT ---
pub fn freeze_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    freeze_account_signed(program, account, mint, authority, &[])
}

pub fn freeze_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::freeze_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::freeze_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(account), mint, authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let is_token_2022 = program.address().as_ref() == ::pinocchio_token_2022::ID.as_ref();
        let ix_token = ::pinocchio_token::instructions::FreezeAccount {
            account: &account.info.view,
            mint: &mint.info.view,
            freeze_authority: &authority.info.view,
            multisig_signers: &[] as &[&crate::prelude::AccountView],
        };
        let ix_2022 = ::pinocchio_token_2022::instructions::FreezeAccount {
            token_program: program.info.view.address(),
            account: &account.info.view,
            mint: &mint.info.view,
            freeze_authority: &authority.info.view,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            if is_token_2022 {
                ix_2022.invoke()?;
            } else {
                ix_token.invoke()?;
            }
        } else {
            if is_token_2022 {
                ix_2022.invoke_signed(signers)?;
            } else {
                ix_token.invoke_signed(signers)?;
            }
        }
        Ok(())
    }
}

// --- THAW ACCOUNT ---
pub fn thaw_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    thaw_account_signed(program, account, mint, authority, &[])
}

pub fn thaw_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::thaw_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::thaw_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(account), mint, authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let is_token_2022 = program.address().as_ref() == ::pinocchio_token_2022::ID.as_ref();
        let ix_token = ::pinocchio_token::instructions::ThawAccount {
            account: &account.info.view,
            mint: &mint.info.view,
            freeze_authority: &authority.info.view,
            multisig_signers: &[] as &[&crate::prelude::AccountView],
        };
        let ix_2022 = ::pinocchio_token_2022::instructions::ThawAccount {
            token_program: program.info.view.address(),
            account: &account.info.view,
            mint: &mint.info.view,
            freeze_authority: &authority.info.view,
        };

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if signers.is_empty() {
            if is_token_2022 {
                ix_2022.invoke()?;
            } else {
                ix_token.invoke()?;
            }
        } else {
            if is_token_2022 {
                ix_2022.invoke_signed(signers)?;
            } else {
                ix_token.invoke_signed(signers)?;
            }
        }
        Ok(())
    }
}

// --- SET AUTHORITY ---
pub fn set_authority(
    program: CpiHandle<'_>,
    account_or_mint: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    authority_type: AuthorityType,
    new_authority: Option<&Address>,
) -> Result<()> {
    set_authority_signed(
        program,
        account_or_mint,
        current_authority,
        authority_type,
        new_authority,
        &[],
    )
}

pub fn set_authority_signed(
    program: CpiHandle<'_>,
    account_or_mint: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    authority_type: AuthorityType,
    new_authority: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            let auth_type = match authority_type {
                AuthorityType::MintTokens => {
                    ::spl_token_2022::instruction::AuthorityType::MintTokens
                }
                AuthorityType::FreezeAccount => {
                    ::spl_token_2022::instruction::AuthorityType::FreezeAccount
                }
                AuthorityType::AccountOwner => {
                    ::spl_token_2022::instruction::AuthorityType::AccountOwner
                }
                AuthorityType::CloseAccount => {
                    ::spl_token_2022::instruction::AuthorityType::CloseAccount
                }
            };
            spl_token_2022::instruction::set_authority(
                &program.address(),
                &account_or_mint.info.address(),
                new_authority,
                auth_type,
                &current_authority.info.address(),
                &[],
            )?
        } else {
            let auth_type = match authority_type {
                AuthorityType::MintTokens => ::spl_token::instruction::AuthorityType::MintTokens,
                AuthorityType::FreezeAccount => {
                    ::spl_token::instruction::AuthorityType::FreezeAccount
                }
                AuthorityType::AccountOwner => {
                    ::spl_token::instruction::AuthorityType::AccountOwner
                }
                AuthorityType::CloseAccount => {
                    ::spl_token::instruction::AuthorityType::CloseAccount
                }
            };
            spl_token::instruction::set_authority(
                &program.address(),
                &account_or_mint.info.address(),
                new_authority,
                auth_type,
                &current_authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(account_or_mint), current_authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let is_token_2022 = program.address().as_ref() == ::pinocchio_token_2022::ID.as_ref();

        // Previously silently truncated any signers/seeds past these
        // hardcoded limits and proceeded with a weaker signer set anyway —
        // now rejected with a clear error instead (mirrors the same fix in
        // naclac-core/src/cpi.rs's invoke_signed_pinocchio(_handles)).
        if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
            return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        for parts in signer_seeds.iter() {
            if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
        }

        let mut signers =
            [const { core::mem::MaybeUninit::<Signer>::uninit() }; crate::prelude::MAX_CPI_SIGNERS];
        let mut all_seeds = [const {
            [const { core::mem::MaybeUninit::<Seed>::uninit() };
                crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
        }; crate::prelude::MAX_CPI_SIGNERS];

        let signer_len = signer_seeds.len();
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len();
                for j in 0..seed_len {
                    let src_seed = *src_signer.get_unchecked(j);
                    let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                    seed_cell.write(Seed::from(src_seed));
                }
                let seeds_slice = core::slice::from_raw_parts(
                    all_seeds.get_unchecked(i).as_ptr() as *const Seed,
                    seed_len,
                );
                let signer_cell = &mut *signers.as_mut_ptr().add(i);
                signer_cell.write(Signer::from(seeds_slice));
            }
        }
        let signers =
            unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

        if is_token_2022 {
            let auth_type = match authority_type {
                AuthorityType::MintTokens => {
                    ::pinocchio_token_2022::instructions::AuthorityType::MintTokens
                }
                AuthorityType::FreezeAccount => {
                    ::pinocchio_token_2022::instructions::AuthorityType::FreezeAccount
                }
                AuthorityType::AccountOwner => {
                    ::pinocchio_token_2022::instructions::AuthorityType::AccountOwner
                }
                AuthorityType::CloseAccount => {
                    ::pinocchio_token_2022::instructions::AuthorityType::CloseAccount
                }
            };
            let ix_2022 = ::pinocchio_token_2022::instructions::SetAuthority {
                token_program: program.info.view.address(),
                account: &account_or_mint.info.view,
                authority: &current_authority.info.view,
                authority_type: auth_type,
                new_authority: new_authority.map(|a| a.as_address()),
            };
            if signers.is_empty() {
                ix_2022.invoke()?;
            } else {
                ix_2022.invoke_signed(signers)?;
            }
        } else {
            let auth_type = match authority_type {
                AuthorityType::MintTokens => {
                    ::pinocchio_token::instructions::AuthorityType::MintTokens
                }
                AuthorityType::FreezeAccount => {
                    ::pinocchio_token::instructions::AuthorityType::FreezeAccount
                }
                AuthorityType::AccountOwner => {
                    ::pinocchio_token::instructions::AuthorityType::AccountOwner
                }
                AuthorityType::CloseAccount => {
                    ::pinocchio_token::instructions::AuthorityType::CloseAccount
                }
            };
            let ix_token = ::pinocchio_token::instructions::SetAuthority {
                account: &account_or_mint.info.view,
                authority: &current_authority.info.view,
                authority_type: auth_type,
                new_authority: new_authority.map(|a| a.as_address()),
                multisig_signers: &[] as &[&crate::prelude::AccountView],
            };
            if signers.is_empty() {
                ix_token.invoke()?;
            } else {
                ix_token.invoke_signed(signers)?;
            }
        }
        Ok(())
    }
}

// ===========================================================================
// TOKEN ACCOUNT & MINT WRAPPERS
// ===========================================================================

#[cfg_attr(
    all(feature = "borsh", not(feature = "pinocchio")),
    derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize)
)]
#[cfg_attr(
    all(feature = "borsh", not(feature = "pinocchio")),
    borsh(crate = "crate::borsh")
)]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TokenAccount(pub [u8; 165]);

#[cfg_attr(
    all(feature = "borsh", not(feature = "pinocchio")),
    derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize)
)]
#[cfg_attr(
    all(feature = "borsh", not(feature = "pinocchio")),
    borsh(crate = "crate::borsh")
)]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Mint(pub [u8; 82]);

// SAFETY: TokenAccount is a transparent wrapper over [u8; 165], which is a POD type.
unsafe impl crate::prelude::Pod for TokenAccount {}
// SAFETY: TokenAccount is a transparent wrapper over [u8; 165], which can be safely zero-initialized.
unsafe impl crate::prelude::Zeroable for TokenAccount {}

// SAFETY: Mint is a transparent wrapper over [u8; 82], which is a POD type.
unsafe impl crate::prelude::Pod for Mint {}
// SAFETY: Mint is a transparent wrapper over [u8; 82], which can be safely zero-initialized.
unsafe impl crate::prelude::Zeroable for Mint {}

impl Discriminator for TokenAccount {
    fn discriminator_len() -> usize {
        0
    }

    /// A plain `TokenAccount` is always owned by the legacy Token program.
    /// `InterfaceAccount<TokenAccount>` (below) is the type to use for a
    /// field that must also accept Token-2022. Beyond the owner check, also
    /// validates the raw layout (exact length, canonical `COption` tags,
    /// initialized state) via `validate_token_account_bytes`.
    fn validate_account(
        info: &crate::prelude::AccountInfo,
        index: usize,
    ) -> crate::prelude::Result<()> {
        if crate::prelude::Owner::program_owner(info) != crate::prelude::TOKEN_PROGRAM_ID {
            return Err(crate::prelude::NaclacError::ConstraintOwner.err(index));
        }
        validate_token_account_bytes(info, index, true)
    }
}

impl Discriminator for Mint {
    fn discriminator_len() -> usize {
        0
    }

    /// See `TokenAccount::validate_account` above; validates the `Mint`
    /// layout via `validate_mint_bytes`.
    fn validate_account(
        info: &crate::prelude::AccountInfo,
        index: usize,
    ) -> crate::prelude::Result<()> {
        if crate::prelude::Owner::program_owner(info) != crate::prelude::TOKEN_PROGRAM_ID {
            return Err(crate::prelude::NaclacError::ConstraintOwner.err(index));
        }
        validate_mint_bytes(info, index, true)
    }
}

impl crate::prelude::NaclacZeroCopy for TokenAccount {}
impl crate::prelude::NaclacZeroCopy for Mint {}

/// `InterfaceAccount<TokenAccount>`/`InterfaceAccount<Mint>` (naclac-core's
/// `wrappers/accounts/interface_account.rs`) accept either the legacy Token
/// program or Token-2022 owner — used directly on `TokenAccount`/`Mint`,
/// with no separate interface-named type, matching real Anchor's own
/// `InterfaceAccount<TokenAccount>` naming exactly. Beyond the owner check
/// naclac-core's `InterfaceAccount<T>` performs itself,
/// `ValidateInterfaceLayout` validates the raw layout permissively (`>=`
/// minimum length, since Token-2022 extensions append a TLV region after
/// the base structure) — canonical `COption` tags, initialized state — via
/// `validate_token_account_layout`/`validate_mint_layout` with
/// `exact_len: false`.
impl crate::prelude::ValidateInterfaceLayout for TokenAccount {
    fn validate_interface_layout(
        data: &[u8],
    ) -> core::result::Result<(), crate::prelude::NaclacError> {
        validate_token_account_layout(data, false)
    }
}

impl crate::prelude::ValidateInterfaceLayout for Mint {
    fn validate_interface_layout(
        data: &[u8],
    ) -> core::result::Result<(), crate::prelude::NaclacError> {
        validate_mint_layout(data, false)
    }
}

/// Reads a `COption<Pubkey>` at `tag_offset` (4-byte tag, then 32-byte
/// value immediately after) — `None` unless the tag is exactly the
/// canonical `[1,0,0,0]` `Some` encoding. Shared by `TokenAccount`'s
/// `delegate`/`close_authority` and `Mint`'s `mint_authority`/
/// `freeze_authority`, all of which use this exact layout.
fn read_coption_address(data: &[u8], tag_offset: usize) -> Option<Address> {
    if data[tag_offset..tag_offset + 4] != [1u8, 0, 0, 0] {
        return None;
    }
    let bytes: [u8; 32] = data[tag_offset + 4..tag_offset + 36].try_into().unwrap();
    Some(Address::new_from_array(bytes))
}

impl TokenAccount {
    pub fn mint(&self) -> Address {
        let bytes = unsafe { *(self.0.as_ptr() as *const [u8; 32]) };
        Address::new_from_array(bytes)
    }
    pub fn owner(&self) -> Address {
        let bytes = unsafe { *(self.0.as_ptr().add(32) as *const [u8; 32]) };
        Address::new_from_array(bytes)
    }
    pub fn amount(&self) -> u64 {
        let bytes = unsafe { *(self.0.as_ptr().add(64) as *const [u8; 8]) };
        u64::from_le_bytes(bytes)
    }
    /// The delegate authorized to transfer up to `delegated_amount()`, if any.
    pub fn delegate(&self) -> Option<Address> {
        read_coption_address(&self.0, 72)
    }
    /// Raw SPL `AccountState` byte: `0` = uninitialized, `1` = initialized, `2` = frozen.
    pub fn state(&self) -> u8 {
        self.0[108]
    }
    /// `true` if `state() == 2` (frozen) — a frozen account rejects `transfer`/`burn`.
    pub fn is_frozen(&self) -> bool {
        self.state() == 2
    }
    /// The rent-exempt reserve if this is a wrapped-SOL account, else `None`.
    pub fn is_native(&self) -> Option<u64> {
        if self.0[109..113] != [1u8, 0, 0, 0] {
            return None;
        }
        let bytes: [u8; 8] = self.0[113..121].try_into().unwrap();
        Some(u64::from_le_bytes(bytes))
    }
    /// The amount `delegate()` is currently authorized to transfer, if any delegate is set.
    pub fn delegated_amount(&self) -> u64 {
        let bytes: [u8; 8] = self.0[121..129].try_into().unwrap();
        u64::from_le_bytes(bytes)
    }
    /// The authority allowed to close this account and reclaim its rent, if any.
    pub fn close_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 129)
    }
}

impl Mint {
    pub fn supply(&self) -> u64 {
        let bytes = unsafe { *(self.0.as_ptr().add(36) as *const [u8; 8]) };
        u64::from_le_bytes(bytes)
    }
    pub fn decimals(&self) -> u8 {
        self.0[44]
    }
    /// The authority allowed to mint new tokens, if the supply isn't fixed.
    pub fn mint_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 0)
    }
    /// The authority allowed to freeze/thaw token accounts of this mint, if any.
    pub fn freeze_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 46)
    }
    /// `true` once the mint has been initialized via `InitializeMint`/`InitializeMint2`.
    pub fn is_initialized(&self) -> bool {
        self.0[45] != 0
    }
}

/// Raw-byte constraint checks backing the `token::*`/`mint::*`/
/// `associated_token::*` attributes in `#[account(...)]`. Each function takes
/// the account's raw data slice rather than a hydrated `&TokenAccount`/`&Mint`,
/// since the generated constraint code runs immediately on `AccountInfo` data,
/// before (and independent of) any typed zero-copy load. This is the single
/// place naclac-macros' codegen defers to for the actual SPL Token/Token-2022
/// layout knowledge — the macro only resolves which expression to compare
/// against and emits the call.
impl TokenAccount {
    /// Compares the account's mint field (offset 0..32) to `expected`.
    pub fn check_mint(data: &[u8], expected: &Address) -> core::result::Result<(), NaclacError> {
        if data.len() < 32 {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        let actual = Address::new_from_array(data[0..32].try_into().unwrap());
        if &actual != expected {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        Ok(())
    }

    /// Compares the account's owner/authority field (offset 32..64) to `expected`.
    pub fn check_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 64 {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        let actual = Address::new_from_array(data[32..64].try_into().unwrap());
        if &actual != expected {
            return Err(NaclacError::ConstraintAddress);
        }
        Ok(())
    }
}

impl Mint {
    /// Compares the mint's decimals byte (offset 44) to `expected`.
    pub fn check_decimals(data: &[u8], expected: u8) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        if data[44] != expected {
            return Err(NaclacError::Unauthorized);
        }
        Ok(())
    }

    /// Reads the mint-authority `COption` (tag at 0..4, address at 4..36):
    /// errors if the authority is `None`, else compares the address to `expected`.
    pub fn check_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        if data[0..4] != [1u8, 0, 0, 0] {
            return Err(NaclacError::Unauthorized);
        }
        let actual = Address::new_from_array(data[4..36].try_into().unwrap());
        if &actual != expected {
            return Err(NaclacError::ConstraintAddress);
        }
        Ok(())
    }

    /// Reads the freeze-authority `COption` (tag at 46..50, address at 50..82):
    /// errors if the freeze authority is `None`, else compares the address to `expected`.
    pub fn check_freeze_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        if data[46..50] != [1u8, 0, 0, 0] {
            return Err(NaclacError::Unauthorized);
        }
        let actual = Address::new_from_array(data[50..82].try_into().unwrap());
        if &actual != expected {
            return Err(NaclacError::ConstraintAddress);
        }
        Ok(())
    }
}

/// True for the two `COption` tag encodings the real SPL Token program ever
/// writes — `[0,0,0,0]` (`None`) or `[1,0,0,0]` (`Some`). Any other 4-byte
/// pattern cannot come from real SPL Token/Token-2022 account data.
fn is_canonical_coption_tag(tag: &[u8]) -> bool {
    tag == [0u8, 0, 0, 0] || tag == [1u8, 0, 0, 0]
}

/// Validates the fixed 165-byte SPL Token `Account` layout beyond
/// owner/discriminator: length (exactly 165 when `exact_len`, else `>= 165`
/// — a Token-2022 account with extensions is legitimately *longer* than 165
/// bytes, a TLV region appended after the base structure, so
/// `InterfaceAccount<TokenAccount>` must not demand exact length the way
/// plain `TokenAccount` does; confirmed against anchor-spl-v2's own split:
/// `token::TokenAccount::validate` uses `require_eq!`, `Interface<TokenAccount>::validate`
/// does not), canonical `COption` tag encoding on
/// `delegate`/`is_native`/`close_authority` (offsets verified against
/// `spl-token-interface`'s real `Account` struct layout, not assumed), and a
/// `state` byte of `Initialized` (1) or `Frozen` (2) — `Uninitialized` (0)
/// is reported distinctly from any other out-of-range value, matching
/// anchor-spl-v2's `validate_token_account_initialized`.
fn validate_token_account_layout(
    data: &[u8],
    exact_len: bool,
) -> core::result::Result<(), NaclacError> {
    let len_ok = if exact_len {
        data.len() == 165
    } else {
        data.len() >= 165
    };
    if !len_ok {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    if !is_canonical_coption_tag(&data[72..76]) // delegate
        || !is_canonical_coption_tag(&data[109..113]) // is_native
        || !is_canonical_coption_tag(&data[129..133])
    // close_authority
    {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    match data[108] {
        0 => Err(NaclacError::AccountNotInitialized),
        1 | 2 => Ok(()),
        _ => Err(NaclacError::InvalidAccountDiscriminator),
    }
}

/// Validates the fixed 82-byte SPL Token `Mint` layout beyond
/// owner/discriminator: length (exactly 82 when `exact_len`, else `>= 82` —
/// see `validate_token_account_layout`'s doc for why `InterfaceAccount<Mint>`
/// needs the non-exact mode), canonical `COption` tag encoding on
/// `mint_authority`/`freeze_authority`, and an `is_initialized` byte of
/// exactly `1` — `0` is reported distinctly, matching
/// `validate_token_account_layout` above.
fn validate_mint_layout(data: &[u8], exact_len: bool) -> core::result::Result<(), NaclacError> {
    let len_ok = if exact_len {
        data.len() == 82
    } else {
        data.len() >= 82
    };
    if !len_ok {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    if !is_canonical_coption_tag(&data[0..4]) // mint_authority
        || !is_canonical_coption_tag(&data[46..50])
    // freeze_authority
    {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    match data[45] {
        0 => Err(NaclacError::AccountNotInitialized),
        1 => Ok(()),
        _ => Err(NaclacError::InvalidAccountDiscriminator),
    }
}

/// Borrows `info`'s raw data and runs `validate_token_account_layout`,
/// converting any failure to a `ProgramError` at `index`. Backend-specific
/// data access only; the actual byte validation is backend-independent.
fn validate_token_account_bytes(
    info: &crate::prelude::AccountInfo,
    index: usize,
    exact_len: bool,
) -> crate::prelude::Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = info.try_borrow_data()?;
        validate_token_account_layout(&data, exact_len).map_err(|e| e.err(index))
    }
    #[cfg(feature = "pinocchio")]
    {
        validate_token_account_layout(info.data(), exact_len).map_err(|e| e.err(index))
    }
}

/// Like `validate_token_account_bytes`, for `validate_mint_layout`.
fn validate_mint_bytes(
    info: &crate::prelude::AccountInfo,
    index: usize,
    exact_len: bool,
) -> crate::prelude::Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = info.try_borrow_data()?;
        validate_mint_layout(&data, exact_len).map_err(|e| e.err(index))
    }
    #[cfg(feature = "pinocchio")]
    {
        validate_mint_layout(info.data(), exact_len).map_err(|e| e.err(index))
    }
}

#[derive(Debug)]
pub struct TransferCheckedAccounts<'a, A, B, C, D> {
    pub from: &'a mut A,
    pub mint: &'a B,
    pub to: &'a mut C,
    pub authority: &'a D,
}

#[derive(Debug)]
pub struct MintToAccounts<'a, A, B, C> {
    pub mint: &'a mut A,
    pub to: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct BurnAccounts<'a, A, B, C> {
    pub mint: &'a mut A,
    pub from: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct CloseAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub destination: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct ApproveAccounts<'a, A, B, C> {
    pub to: &'a mut A,
    pub delegate: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct RevokeAccounts<'a, A, B> {
    pub source: &'a mut A,
    pub authority: &'a B,
}

#[derive(Debug)]
pub struct InitializeMintAccounts<'a, A> {
    pub mint: &'a mut A,
}

#[derive(Debug)]
pub struct SyncNativeAccounts<'a, A> {
    pub native_token: &'a mut A,
}

#[derive(Debug)]
pub struct InitializeAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct FreezeAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct ThawAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct SetAuthorityAccounts<'a, A, B> {
    pub account_or_mint: &'a mut A,
    pub current_authority: &'a B,
}

pub trait TokenCpi {
    fn transfer_checked<'a, A, B, C, D>(
        &self,
        accounts: TransferCheckedAccounts<'a, A, B, C, D>,
        amount: u64,
        decimals: u8,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandleMut<'a>,
        D: ToCpiHandle<'a>;

    fn transfer_checked_signed<'a, A, B, C, D>(
        &self,
        accounts: TransferCheckedAccounts<'a, A, B, C, D>,
        amount: u64,
        decimals: u8,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandleMut<'a>,
        D: ToCpiHandle<'a>;

    fn mint_to<'a, A, B, C>(
        &self,
        accounts: MintToAccounts<'a, A, B, C>,
        amount: u64,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn mint_to_signed<'a, A, B, C>(
        &self,
        accounts: MintToAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn burn<'a, A, B, C>(&self, accounts: BurnAccounts<'a, A, B, C>, amount: u64) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn burn_signed<'a, A, B, C>(
        &self,
        accounts: BurnAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn close_account<'a, A, B, C>(&self, accounts: CloseAccountAccounts<'a, A, B, C>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn close_account_signed<'a, A, B, C>(
        &self,
        accounts: CloseAccountAccounts<'a, A, B, C>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    /// Permissionless (no authority to sign for) — no `_signed` counterpart.
    fn sync_native<'a, A>(&self, accounts: SyncNativeAccounts<'a, A>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn approve<'a, A, B, C>(
        &self,
        accounts: ApproveAccounts<'a, A, B, C>,
        amount: u64,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn approve_signed<'a, A, B, C>(
        &self,
        accounts: ApproveAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn revoke<'a, A, B>(&self, accounts: RevokeAccounts<'a, A, B>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn revoke_signed<'a, A, B>(
        &self,
        accounts: RevokeAccounts<'a, A, B>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn initialize_mint<'a, A>(
        &self,
        accounts: InitializeMintAccounts<'a, A>,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn initialize_mint_signed<'a, A>(
        &self,
        accounts: InitializeMintAccounts<'a, A>,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn initialize_account<'a, A, B, C>(
        &self,
        accounts: InitializeAccountAccounts<'a, A, B, C>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn initialize_account_signed<'a, A, B, C>(
        &self,
        accounts: InitializeAccountAccounts<'a, A, B, C>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn freeze_account<'a, A, B, C>(
        &self,
        accounts: FreezeAccountAccounts<'a, A, B, C>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn freeze_account_signed<'a, A, B, C>(
        &self,
        accounts: FreezeAccountAccounts<'a, A, B, C>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn thaw_account<'a, A, B, C>(&self, accounts: ThawAccountAccounts<'a, A, B, C>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn thaw_account_signed<'a, A, B, C>(
        &self,
        accounts: ThawAccountAccounts<'a, A, B, C>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn set_authority<'a, A, B>(
        &self,
        accounts: SetAuthorityAccounts<'a, A, B>,
        authority_type: AuthorityType,
        new_authority: Option<&Address>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn set_authority_signed<'a, A, B>(
        &self,
        accounts: SetAuthorityAccounts<'a, A, B>,
        authority_type: AuthorityType,
        new_authority: Option<&Address>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;
}

macro_rules! impl_token_cpi_helpers {
    ($type:ty) => {
        impl TokenCpi for $type {
            fn transfer_checked<'a, A, B, C, D>(
                &self,
                accounts: TransferCheckedAccounts<'a, A, B, C, D>,
                amount: u64,
                decimals: u8,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandleMut<'a>,
                D: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer_checked(
                    program_h,
                    from_h,
                    mint_h,
                    to_h,
                    authority_h,
                    amount,
                    decimals,
                )
            }

            fn transfer_checked_signed<'a, A, B, C, D>(
                &self,
                accounts: TransferCheckedAccounts<'a, A, B, C, D>,
                amount: u64,
                decimals: u8,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandleMut<'a>,
                D: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer_checked_signed(
                    program_h,
                    from_h,
                    mint_h,
                    to_h,
                    authority_h,
                    CheckedTransferParams { amount, decimals },
                    signer_seeds,
                )
            }

            fn mint_to<'a, A, B, C>(
                &self,
                accounts: MintToAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                mint_to(program_h, mint_h, to_h, authority_h, amount)
            }

            fn mint_to_signed<'a, A, B, C>(
                &self,
                accounts: MintToAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                mint_to_signed(program_h, mint_h, to_h, authority_h, amount, signer_seeds)
            }

            fn burn<'a, A, B, C>(
                &self,
                accounts: BurnAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let from_h = accounts.from.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                burn(program_h, mint_h, from_h, authority_h, amount)
            }

            fn burn_signed<'a, A, B, C>(
                &self,
                accounts: BurnAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let from_h = accounts.from.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                burn_signed(program_h, mint_h, from_h, authority_h, amount, signer_seeds)
            }

            fn close_account<'a, A, B, C>(
                &self,
                accounts: CloseAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let destination_h = accounts.destination.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                close_account(program_h, account_h, destination_h, authority_h)
            }

            fn close_account_signed<'a, A, B, C>(
                &self,
                accounts: CloseAccountAccounts<'a, A, B, C>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let destination_h = accounts.destination.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                close_account_signed(
                    program_h,
                    account_h,
                    destination_h,
                    authority_h,
                    signer_seeds,
                )
            }

            fn sync_native<'a, A>(&self, accounts: SyncNativeAccounts<'a, A>) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let native_token_h = accounts.native_token.to_cpi_handle_mut();
                sync_native(program_h, native_token_h)
            }

            fn initialize_mint<'a, A>(
                &self,
                accounts: InitializeMintAccounts<'a, A>,
                decimals: u8,
                mint_authority: &Address,
                freeze_authority: Option<&Address>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                initialize_mint(
                    program_h,
                    mint_h,
                    decimals,
                    mint_authority,
                    freeze_authority,
                )
            }

            fn initialize_mint_signed<'a, A>(
                &self,
                accounts: InitializeMintAccounts<'a, A>,
                decimals: u8,
                mint_authority: &Address,
                freeze_authority: Option<&Address>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                initialize_mint_signed(
                    program_h,
                    mint_h,
                    decimals,
                    mint_authority,
                    freeze_authority,
                    signer_seeds,
                )
            }

            fn approve<'a, A, B, C>(
                &self,
                accounts: ApproveAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let delegate_h = accounts.delegate.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                approve(program_h, to_h, delegate_h, authority_h, amount)
            }

            fn approve_signed<'a, A, B, C>(
                &self,
                accounts: ApproveAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let delegate_h = accounts.delegate.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                approve_signed(
                    program_h,
                    to_h,
                    delegate_h,
                    authority_h,
                    amount,
                    signer_seeds,
                )
            }

            fn revoke<'a, A, B>(&self, accounts: RevokeAccounts<'a, A, B>) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let source_h = accounts.source.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                revoke(program_h, source_h, authority_h)
            }

            fn revoke_signed<'a, A, B>(
                &self,
                accounts: RevokeAccounts<'a, A, B>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let source_h = accounts.source.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                revoke_signed(program_h, source_h, authority_h, signer_seeds)
            }

            fn initialize_account<'a, A, B, C>(
                &self,
                accounts: InitializeAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                initialize_account(program_h, account_h, mint_h, authority_h)
            }

            fn initialize_account_signed<'a, A, B, C>(
                &self,
                accounts: InitializeAccountAccounts<'a, A, B, C>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                initialize_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn freeze_account<'a, A, B, C>(
                &self,
                accounts: FreezeAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                freeze_account(program_h, account_h, mint_h, authority_h)
            }

            fn freeze_account_signed<'a, A, B, C>(
                &self,
                accounts: FreezeAccountAccounts<'a, A, B, C>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                freeze_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn thaw_account<'a, A, B, C>(
                &self,
                accounts: ThawAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                thaw_account(program_h, account_h, mint_h, authority_h)
            }

            fn thaw_account_signed<'a, A, B, C>(
                &self,
                accounts: ThawAccountAccounts<'a, A, B, C>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                thaw_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn set_authority<'a, A, B>(
                &self,
                accounts: SetAuthorityAccounts<'a, A, B>,
                authority_type: AuthorityType,
                new_authority: Option<&Address>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_or_mint_h = accounts.account_or_mint.to_cpi_handle_mut();
                let current_authority_h = accounts.current_authority.to_cpi_handle();
                set_authority(
                    program_h,
                    account_or_mint_h,
                    current_authority_h,
                    authority_type,
                    new_authority,
                )
            }

            fn set_authority_signed<'a, A, B>(
                &self,
                accounts: SetAuthorityAccounts<'a, A, B>,
                authority_type: AuthorityType,
                new_authority: Option<&Address>,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_or_mint_h = accounts.account_or_mint.to_cpi_handle_mut();
                let current_authority_h = accounts.current_authority.to_cpi_handle();
                set_authority_signed(
                    program_h,
                    account_or_mint_h,
                    current_authority_h,
                    authority_type,
                    new_authority,
                    signer_seeds,
                )
            }
        }
    };
}

impl_token_cpi_helpers!(Program<Token>);
impl_token_cpi_helpers!(Program<Token2022>);
impl_token_cpi_helpers!(Interface<TokenInterface>);
