//! # SPL Token & Token-2022 CPI Abstractions
//!
//! Provides a unified interface for interacting with SPL Token programs across both
//! standard Solana (using spl-token and spl-token-2022 crates) and Pinocchio 
//! (using pinocchio-token and pinocchio-token-2022). 
//! 
//! In Pinocchio mode, it extensively uses MaybeUninit stack arrays and pointer 
//! slicing to generate PDA signers with zero heap allocations.
use crate::prelude::{Result, Pubkey, Discriminator, CpiContext};
#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::{Key, ToAccountInfo};

#[cfg(not(feature = "pinocchio"))]
use solana_program::program::invoke_signed;


// ===========================================================================
// --- PINOCCHIO DEPS ---
// ===========================================================================

#[cfg(feature = "pinocchio")]
use pinocchio::cpi::{Signer, Seed};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum AuthorityType {
    MintTokens,
    FreezeAccount,
    AccountOwner,
    CloseAccount,
}

// ===========================================================================
// --- TRANSFER ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct Transfer<'info> {
    pub from: solana_program::account_info::AccountInfo<'info>,
    pub to: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct Transfer<'info> {
    pub from: crate::prelude::AccountInfo<'info>,
    pub to: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub struct TransferChecked<'info> {
    pub from: solana_program::account_info::AccountInfo<'info>,
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub to: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct TransferChecked<'info> {
    pub from: crate::prelude::AccountInfo<'info>,
    pub mint: crate::prelude::AccountInfo<'info>,
    pub to: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
#[allow(deprecated)]
pub fn transfer<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::transfer(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount)?
    } else {
        spl_token::instruction::transfer(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount)?
    };
    invoke_signed(&ix, &[ctx.accounts.from.clone(), ctx.accounts.to.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(not(feature = "pinocchio"))]
pub fn transfer_checked<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::transfer_checked(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.mint.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount, decimals)?
    } else {
        spl_token::instruction::transfer_checked(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.mint.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount, decimals)?
    };
    invoke_signed(&ix, &[ctx.accounts.from.clone(), ctx.accounts.mint.clone(), ctx.accounts.to.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
#[allow(deprecated)]
pub fn transfer<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::Transfer {
        token_program: ctx.program.view.address(),
        from: &ctx.accounts.from.view,
        to: &ctx.accounts.to.view,
        authority: &ctx.accounts.authority.view,
        amount,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

#[cfg(feature = "pinocchio")]
pub fn transfer_checked<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::TransferChecked {
        token_program: ctx.program.view.address(),
        from: &ctx.accounts.from.view,
        mint: &ctx.accounts.mint.view,
        to: &ctx.accounts.to.view,
        authority: &ctx.accounts.authority.view,
        amount,
        decimals,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- MINT TO ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct MintTo<'info> {
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub to: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct MintTo<'info> {
    pub mint: crate::prelude::AccountInfo<'info>,
    pub to: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn mint_to<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, MintTo<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::mint_to(&ctx.program.key(), &ctx.accounts.mint.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount)?
    } else {
        spl_token::instruction::mint_to(&ctx.program.key(), &ctx.accounts.mint.key(), &ctx.accounts.to.key(), &ctx.accounts.authority.key(), &[], amount)?
    };
    invoke_signed(&ix, &[ctx.accounts.mint.clone(), ctx.accounts.to.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn mint_to<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, MintTo<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::MintTo {
        token_program: ctx.program.view.address(),
        mint: &ctx.accounts.mint.view,
        account: &ctx.accounts.to.view,
        mint_authority: &ctx.accounts.authority.view,
        amount,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- BURN ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct Burn<'info> {
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub from: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct Burn<'info> {
    pub mint: crate::prelude::AccountInfo<'info>,
    pub from: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn burn<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Burn<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::burn(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[], amount)?
    } else {
        spl_token::instruction::burn(&ctx.program.key(), &ctx.accounts.from.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[], amount)?
    };
    invoke_signed(&ix, &[ctx.accounts.from.clone(), ctx.accounts.mint.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn burn<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Burn<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::Burn {
        token_program: ctx.program.view.address(),
        account: &ctx.accounts.from.view,
        mint: &ctx.accounts.mint.view,
        authority: &ctx.accounts.authority.view,
        amount,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- CLOSE ACCOUNT ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct CloseAccount<'info> {
    pub account: solana_program::account_info::AccountInfo<'info>,
    pub destination: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct CloseAccount<'info> {
    pub account: crate::prelude::AccountInfo<'info>,
    pub destination: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn close_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, CloseAccount<'info>>,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::close_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.destination.key(), &ctx.accounts.authority.key(), &[])?
    } else {
        spl_token::instruction::close_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.destination.key(), &ctx.accounts.authority.key(), &[])?
    };
    invoke_signed(&ix, &[ctx.accounts.account.clone(), ctx.accounts.destination.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn close_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, CloseAccount<'info>>,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::CloseAccount {
        token_program: ctx.program.view.address(),
        account: &ctx.accounts.account.view,
        destination: &ctx.accounts.destination.view,
        authority: &ctx.accounts.authority.view,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- APPROVE ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct Approve<'info> {
    pub to: solana_program::account_info::AccountInfo<'info>,
    pub delegate: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct Approve<'info> {
    pub to: crate::prelude::AccountInfo<'info>,
    pub delegate: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn approve<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Approve<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::approve(&ctx.program.key(), &ctx.accounts.to.key(), &ctx.accounts.delegate.key(), &ctx.accounts.authority.key(), &[], amount)?
    } else {
        spl_token::instruction::approve(&ctx.program.key(), &ctx.accounts.to.key(), &ctx.accounts.delegate.key(), &ctx.accounts.authority.key(), &[], amount)?
    };
    invoke_signed(&ix, &[ctx.accounts.to.clone(), ctx.accounts.delegate.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn approve<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Approve<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::Approve {
        token_program: ctx.program.view.address(),
        source: &ctx.accounts.to.view,
        delegate: &ctx.accounts.delegate.view,
        authority: &ctx.accounts.authority.view,
        amount,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- REVOKE ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct Revoke<'info> {
    pub source: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct Revoke<'info> {
    pub source: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn revoke<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Revoke<'info>>,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::revoke(&ctx.program.key(), &ctx.accounts.source.key(), &ctx.accounts.authority.key(), &[])?
    } else {
        spl_token::instruction::revoke(&ctx.program.key(), &ctx.accounts.source.key(), &ctx.accounts.authority.key(), &[])?
    };
    invoke_signed(&ix, &[ctx.accounts.source.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn revoke<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Revoke<'info>>,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::Revoke {
        token_program: ctx.program.view.address(),
        source: &ctx.accounts.source.view,
        authority: &ctx.accounts.authority.view,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        ix.invoke().map_err(Into::into)
    } else {
        ix.invoke_signed(signers).map_err(Into::into)
    }
}

// ===========================================================================
// --- INITIALIZE MINT ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct InitializeMint<'info> {
    pub mint: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct InitializeMint<'info> {
    pub mint: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn initialize_mint<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, InitializeMint<'info>>,
    decimals: u8,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
) -> Result<()> {
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
        program_id: ctx.program.key(),
        accounts: vec![
            solana_program::instruction::AccountMeta::new(ctx.accounts.mint.key(), false),
        ],
        data,
    };
    invoke_signed(&ix, &[ctx.accounts.mint.clone(), ctx.program.to_account_info()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn initialize_mint<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, InitializeMint<'info>>,
    decimals: u8,
    mint_authority: &crate::prelude::Pubkey,
    freeze_authority: Option<&crate::prelude::Pubkey>,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::InitializeMint2 {
        token_program: ctx.program.view.address(),
        mint: &ctx.accounts.mint.view,
        decimals,
        mint_authority: mint_authority.as_address(),
        freeze_authority: freeze_authority.map(|a| a.as_address()),
    };

    ix.invoke().map_err(Into::into)
}

// ===========================================================================
// --- INITIALIZE ACCOUNT ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct InitializeAccount<'info> {
    pub account: solana_program::account_info::AccountInfo<'info>,
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct InitializeAccount<'info> {
    pub account: crate::prelude::AccountInfo<'info>,
    pub mint: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn initialize_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, InitializeAccount<'info>>,
) -> Result<()> {
    let mut data = [0u8; 33];
    data[0] = 18; // InitializeAccount3 discriminant
    data[1..].copy_from_slice(ctx.accounts.authority.key().as_ref());

    let ix = solana_program::instruction::Instruction {
        program_id: ctx.program.key(),
        accounts: vec![
            solana_program::instruction::AccountMeta::new(ctx.accounts.account.key(), false),
            solana_program::instruction::AccountMeta::new_readonly(ctx.accounts.mint.key(), false),
        ],
        data: data.to_vec(),
    };
    invoke_signed(&ix, &[ctx.accounts.account.clone(), ctx.accounts.mint.clone(), ctx.program.to_account_info()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn initialize_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, InitializeAccount<'info>>,
) -> Result<()> {
    let ix = ::pinocchio_token_2022::instructions::InitializeAccount3 {
        token_program: ctx.program.view.address(),
        account: &ctx.accounts.account.view,
        mint: &ctx.accounts.mint.view,
        owner: ctx.accounts.authority.view.address(),
    };

    ix.invoke().map_err(Into::into)
}

// ===========================================================================
// --- FREEZE ACCOUNT ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct FreezeAccount<'info> {
    pub account: solana_program::account_info::AccountInfo<'info>,
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct FreezeAccount<'info> {
    pub account: crate::prelude::AccountInfo<'info>,
    pub mint: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn freeze_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, FreezeAccount<'info>>,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::freeze_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[])?
    } else {
        spl_token::instruction::freeze_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[])?
    };
    invoke_signed(&ix, &[ctx.accounts.account.clone(), ctx.accounts.mint.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn freeze_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, FreezeAccount<'info>>,
) -> Result<()> {
    let is_token_2022 = ctx.program.key().as_ref() == ::pinocchio_token_2022::ID.as_ref();
    let ix_token = ::pinocchio_token::instructions::FreezeAccount {
        account: &ctx.accounts.account.view,
        mint: &ctx.accounts.mint.view,
        freeze_authority: &ctx.accounts.authority.view,
    };
    let ix_2022 = ::pinocchio_token_2022::instructions::FreezeAccount {
        token_program: ctx.program.view.address(),
        account: &ctx.accounts.account.view,
        mint: &ctx.accounts.mint.view,
        freeze_authority: &ctx.accounts.authority.view,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        if is_token_2022 { ix_2022.invoke().map_err(Into::into) } else { ix_token.invoke().map_err(Into::into) }
    } else {
        if is_token_2022 { ix_2022.invoke_signed(signers).map_err(Into::into) } else { ix_token.invoke_signed(signers).map_err(Into::into) }
    }
}

// ===========================================================================
// --- THAW ACCOUNT ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct ThawAccount<'info> {
    pub account: solana_program::account_info::AccountInfo<'info>,
    pub mint: solana_program::account_info::AccountInfo<'info>,
    pub authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct ThawAccount<'info> {
    pub account: crate::prelude::AccountInfo<'info>,
    pub mint: crate::prelude::AccountInfo<'info>,
    pub authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn thaw_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, ThawAccount<'info>>,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        spl_token_2022::instruction::thaw_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[])?
    } else {
        spl_token::instruction::thaw_account(&ctx.program.key(), &ctx.accounts.account.key(), &ctx.accounts.mint.key(), &ctx.accounts.authority.key(), &[])?
    };
    invoke_signed(&ix, &[ctx.accounts.account.clone(), ctx.accounts.mint.clone(), ctx.accounts.authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn thaw_account<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, ThawAccount<'info>>,
) -> Result<()> {
    let is_token_2022 = ctx.program.key().as_ref() == ::pinocchio_token_2022::ID.as_ref();
    let ix_token = ::pinocchio_token::instructions::ThawAccount {
        account: &ctx.accounts.account.view,
        mint: &ctx.accounts.mint.view,
        freeze_authority: &ctx.accounts.authority.view,
    };
    let ix_2022 = ::pinocchio_token_2022::instructions::ThawAccount {
        token_program: ctx.program.view.address(),
        account: &ctx.accounts.account.view,
        mint: &ctx.accounts.mint.view,
        freeze_authority: &ctx.accounts.authority.view,
    };

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if signers.is_empty() {
        if is_token_2022 { ix_2022.invoke().map_err(Into::into) } else { ix_token.invoke().map_err(Into::into) }
    } else {
        if is_token_2022 { ix_2022.invoke_signed(signers).map_err(Into::into) } else { ix_token.invoke_signed(signers).map_err(Into::into) }
    }
}

// ===========================================================================
// --- SET AUTHORITY ---
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub struct SetAuthority<'info> {
    pub account_or_mint: solana_program::account_info::AccountInfo<'info>,
    pub current_authority: solana_program::account_info::AccountInfo<'info>,
}

#[cfg(feature = "pinocchio")]
pub struct SetAuthority<'info> {
    pub account_or_mint: crate::prelude::AccountInfo<'info>,
    pub current_authority: crate::prelude::AccountInfo<'info>,
}

#[cfg(not(feature = "pinocchio"))]
pub fn set_authority<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, SetAuthority<'info>>,
    authority_type: AuthorityType,
    new_authority: Option<&Pubkey>,
) -> Result<()> {
    let ix = if ctx.program.key() == spl_token_2022::ID {
        let auth_type = match authority_type {
            AuthorityType::MintTokens => ::spl_token_2022::instruction::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => ::spl_token_2022::instruction::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => ::spl_token_2022::instruction::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => ::spl_token_2022::instruction::AuthorityType::CloseAccount,
        };
        spl_token_2022::instruction::set_authority(&ctx.program.key(), &ctx.accounts.account_or_mint.key(), new_authority, auth_type, &ctx.accounts.current_authority.key(), &[])?
    } else {
        let auth_type = match authority_type {
            AuthorityType::MintTokens => ::spl_token::instruction::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => ::spl_token::instruction::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => ::spl_token::instruction::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => ::spl_token::instruction::AuthorityType::CloseAccount,
        };
        spl_token::instruction::set_authority(&ctx.program.key(), &ctx.accounts.account_or_mint.key(), new_authority, auth_type, &ctx.accounts.current_authority.key(), &[])?
    };
    invoke_signed(&ix, &[ctx.accounts.account_or_mint.clone(), ctx.accounts.current_authority.clone()], ctx.signer_seeds)
}

#[cfg(feature = "pinocchio")]
pub fn set_authority<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, SetAuthority<'info>>,
    authority_type: AuthorityType,
    new_authority: Option<&crate::prelude::Pubkey>,
) -> Result<()> {
    let is_token_2022 = ctx.program.key().as_ref() == ::pinocchio_token_2022::ID.as_ref();

    let mut signers = [const { core::mem::MaybeUninit::<Signer>::uninit() }; 4];
    let mut all_seeds = [const { [const { core::mem::MaybeUninit::<Seed>::uninit() }; 8] }; 4];
    
    let signer_len = ctx.signer_seeds.len().min(4);
    for i in 0..signer_len {
        let seed_len = ctx.signer_seeds[i].len().min(8);
        for j in 0..seed_len {
            all_seeds[i][j].write(Seed::from(ctx.signer_seeds[i][j]));
        }
// SAFETY: The MaybeUninit arrays have been explicitly initialized up to seed_len. 
        // It is sound to cast the valid subset of the array pointer to a slice of Seeds.
        let seeds_slice = unsafe { core::slice::from_raw_parts(all_seeds[i].as_ptr() as *const Seed, seed_len) };
        signers[i].write(Signer::from(seeds_slice));
    }
// SAFETY: The MaybeUninit array has been explicitly initialized up to signer_len. 
    // It is sound to cast the valid subset of the array pointer to a slice of Signers.
    let signers = unsafe { core::slice::from_raw_parts(signers.as_ptr() as *const Signer, signer_len) };

    if is_token_2022 {
        let auth_type = match authority_type {
            AuthorityType::MintTokens => ::pinocchio_token_2022::instructions::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => ::pinocchio_token_2022::instructions::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => ::pinocchio_token_2022::instructions::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => ::pinocchio_token_2022::instructions::AuthorityType::CloseAccount,
        };
        let ix_2022 = ::pinocchio_token_2022::instructions::SetAuthority {
            token_program: ctx.program.view.address(),
            account: &ctx.accounts.account_or_mint.view,
            authority: &ctx.accounts.current_authority.view,
            authority_type: auth_type,
            new_authority: new_authority.map(|a| a.as_address()),
        };
        if signers.is_empty() { ix_2022.invoke().map_err(Into::into) } else { ix_2022.invoke_signed(signers).map_err(Into::into) }
    } else {
        let auth_type = match authority_type {
            AuthorityType::MintTokens => ::pinocchio_token::instructions::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => ::pinocchio_token::instructions::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => ::pinocchio_token::instructions::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => ::pinocchio_token::instructions::AuthorityType::CloseAccount,
        };
        let ix_token = ::pinocchio_token::instructions::SetAuthority {
            account: &ctx.accounts.account_or_mint.view,
            authority: &ctx.accounts.current_authority.view,
            authority_type: auth_type,
            new_authority: new_authority.map(|a| a.as_address()),
        };
        if signers.is_empty() { ix_token.invoke().map_err(Into::into) } else { ix_token.invoke_signed(signers).map_err(Into::into) }
    }
}

// ===========================================================================
// TOKEN ACCOUNT & MINT WRAPPERS
// ===========================================================================

#[cfg_attr(feature = "borsh", derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize))]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TokenAccount(pub [u8; 165]);

#[cfg_attr(feature = "borsh", derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize))]
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
    fn discriminator_len() -> usize { 0 }
}

impl Discriminator for Mint {
    fn discriminator_len() -> usize { 0 }
}

impl crate::prelude::NaclacZeroCopy for TokenAccount {}
impl crate::prelude::NaclacZeroCopy for Mint {}

impl TokenAccount {
    pub fn mint(&self) -> Pubkey { Pubkey::new_from_array(self.0[0..32].try_into().unwrap()) }
    pub fn owner(&self) -> Pubkey { Pubkey::new_from_array(self.0[32..64].try_into().unwrap()) }
    pub fn amount(&self) -> u64 { u64::from_le_bytes(self.0[64..72].try_into().unwrap()) }
}

impl Mint {
    pub fn supply(&self) -> u64 { u64::from_le_bytes(self.0[36..44].try_into().unwrap()) }
    pub fn decimals(&self) -> u8 { self.0[44] }
}
