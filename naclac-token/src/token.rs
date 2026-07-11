// ===========================================================================
// token.rs — SPL Token & Token-2022 CPI helpers
// ===========================================================================

//! # SPL Token & Token-2022 CPI Abstractions
//!
//! Provides a unified interface for interacting with SPL Token programs across both
//! standard Solana and Pinocchio, using borrow-checked CPI handles.

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Discriminator, Result};
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

// --- TRANSFER ---
pub fn transfer(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    transfer_signed(program, from, to, authority, amount, &[])
}

pub fn transfer_signed(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            #[allow(deprecated)]
            let ix_2022 = spl_token_2022::instruction::transfer(
                &program.address(),
                &from.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?;
            ix_2022
        } else {
            spl_token::instruction::transfer(
                &program.address(),
                &from.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [
            CpiHandle::from(from),
            CpiHandle::from(to),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::Transfer {
            token_program: program.info.view.address(),
            from: &from.info.view,
            to: &to.info.view,
            authority: &authority.info.view,
            amount,
        };

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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
        let mut data = vec![16u8]; // InitializeMint2
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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

        let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
        let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];

        let signer_len = signer_seeds.len().min(4);
        for i in 0..signer_len {
            unsafe {
                let src_signer = *signer_seeds.get_unchecked(i);
                let seed_len = src_signer.len().min(8);
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
}

impl Discriminator for Mint {
    fn discriminator_len() -> usize {
        0
    }
}

impl crate::prelude::NaclacZeroCopy for TokenAccount {}
impl crate::prelude::NaclacZeroCopy for Mint {}

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
}

impl Mint {
    pub fn supply(&self) -> u64 {
        let bytes = unsafe { *(self.0.as_ptr().add(36) as *const [u8; 8]) };
        u64::from_le_bytes(bytes)
    }
    pub fn decimals(&self) -> u8 {
        self.0[44]
    }
}

#[derive(Debug)]
pub struct TransferAccounts<'a, A, B, C> {
    pub from: &'a mut A,
    pub to: &'a mut B,
    pub authority: &'a C,
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
    fn transfer<'a, A, B, C>(
        &self,
        accounts: TransferAccounts<'a, A, B, C>,
        amount: u64,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn transfer_signed<'a, A, B, C>(
        &self,
        accounts: TransferAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

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
            fn transfer<'a, A, B, C>(
                &self,
                accounts: TransferAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer(program_h, from_h, to_h, authority_h, amount)
            }

            fn transfer_signed<'a, A, B, C>(
                &self,
                accounts: TransferAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[&[&[u8]]],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer_signed(program_h, from_h, to_h, authority_h, amount, signer_seeds)
            }

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
