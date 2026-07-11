// ===========================================================================
// cpi.rs — Standardized Cross-Program Invocation (CPI) module
// ===========================================================================

//! # Cross-Program Invocation (CPI) Execution
//!
//! Provides the borrow-checked execution helpers for issuing Cross-Program Invocations.
//! This handles the compilation targets for Solana (via AccountInfo conversion)
//! and Pinocchio (via zero-overhead transmutations).

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

// SOLANA-PROGRAM BACKEND (invoke / invoke_signed)
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

// PINOCCHIO BACKEND (invoke_pinocchio / invoke_signed_pinocchio_handles)
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
    let account_infos: &[crate::prelude::AccountInfo] =
        unsafe { core::mem::transmute(account_handles) };

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() }; 32];
    let cpi_accounts_len = account_infos.len().min(32);
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
        let mut signers: [::pinocchio::cpi::Signer; crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut seeds_buffer: [::pinocchio::cpi::Seed;
            crate::prelude::MAX_CPI_SEEDS_PER_SIGNER * crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut seed_ranges = [(0usize, 0usize); crate::prelude::MAX_CPI_SIGNERS];

        let mut seed_idx = 0;
        let mut signer_idx = 0;

        for (i, seed_parts) in signer_seeds.iter().enumerate() {
            if i >= signers.len() {
                break;
            }
            let start_seed = seed_idx;
            for part in seed_parts.iter() {
                if seed_idx >= seeds_buffer.len() {
                    break;
                }
                seeds_buffer[seed_idx] = ::pinocchio::cpi::Seed::from(*part);
                seed_idx += 1;
            }
            seed_ranges[i] = (start_seed, seed_idx);
            signer_idx += 1;
        }

        for i in 0..signer_idx {
            let (start, end) = seed_ranges[i];
            signers[i] = ::pinocchio::cpi::Signer::from(&seeds_buffer[start..end]);
        }

        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(
                instruction,
                cpi_accounts,
                &signers[..signer_idx],
            );
        }
    }
    Ok(())
}

/// Kept for backward compatibility with off-chain client SDKs.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() }; 32];
    let cpi_accounts_len = account_infos.len().min(32);
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
        let mut signers: [::pinocchio::cpi::Signer; crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut seeds_buffer: [::pinocchio::cpi::Seed;
            crate::prelude::MAX_CPI_SEEDS_PER_SIGNER * crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut seed_ranges = [(0usize, 0usize); crate::prelude::MAX_CPI_SIGNERS];

        let mut seed_idx = 0;
        let mut signer_idx = 0;

        for (i, seed_parts) in signer_seeds.iter().enumerate() {
            if i >= signers.len() {
                break;
            }
            let start_seed = seed_idx;
            for part in seed_parts.iter() {
                if seed_idx >= seeds_buffer.len() {
                    break;
                }
                seeds_buffer[seed_idx] = ::pinocchio::cpi::Seed::from(*part);
                seed_idx += 1;
            }
            seed_ranges[i] = (start_seed, seed_idx);
            signer_idx += 1;
        }

        for i in 0..signer_idx {
            let (start, end) = seed_ranges[i];
            signers[i] = ::pinocchio::cpi::Signer::from(&seeds_buffer[start..end]);
        }

        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(
                instruction,
                cpi_accounts,
                &signers[..signer_idx],
            );
        }
    }
    Ok(())
}
