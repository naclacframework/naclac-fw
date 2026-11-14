//! `SyncNative`: recomputes a native (WSOL) token account's amount from its
//! lamport balance, with a variant that pads extra accounts onto the CPI.

use crate::prelude::{CpiHandle, CpiHandleMut, NaclacError, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};

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
        let native_token = CpiHandle::from(native_token);
        let accounts = [InstructionAccount::writable(
            native_token.info.view.address(),
        )];
        invoke(&program, &accounts, &[native_token], &[17], &[])
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
            solana_instruction::AccountMeta::new_readonly(rent_sysvar.address(), false),
        );
        ix.accounts.extend(
            extra_accounts
                .iter()
                .map(|extra| solana_instruction::AccountMeta::new(extra.address(), false)),
        );

        let native_handle: CpiHandle<'_> = CpiHandle::from(native_token);
        let mut accounts: [CpiHandle<'_>; MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 3] =
            [native_handle; MAX_SYNC_NATIVE_EXTRA_ACCOUNTS + 3];
        accounts[1] = rent_sysvar;
        accounts[2] = program;
        let extra_len = extra_accounts.len();
        accounts[3..3 + extra_len].copy_from_slice(extra_accounts);

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
