//! # Associated Token Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking the SPL Associated Token Program.
//! In Pinocchio mode, this avoids standard Borsh serialization and heap allocations 
//! by constructing PDA signer seeds entirely on the stack.

use crate::cpi::CpiContext;
use crate::prelude::{Result, AccountInfo};

pub struct Create<'info> {
    pub payer: AccountInfo<'info>,
    pub associated_token: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
}

pub fn create<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, Create<'info>>,
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            ctx.accounts.payer.key,
            ctx.accounts.authority.key,
            ctx.accounts.mint.key,
            ctx.accounts.token_program.key, 
        );

        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.payer.clone(),
                ctx.accounts.associated_token.clone(),
                ctx.accounts.authority.clone(),
                ctx.accounts.mint.clone(),
                ctx.accounts.system_program.clone(),
                ctx.accounts.token_program.clone(),
                ctx.program.clone(),
            ],
            ctx.signer_seeds,
        ).map_err(Into::into)
    }

    #[cfg(feature = "pinocchio")]
    {
        use pinocchio_associated_token_account::instructions::Create as PinocchioCreate;
        
        let ix = PinocchioCreate {
            funding_account: &ctx.accounts.payer,
            account: &ctx.accounts.associated_token,
            wallet: &ctx.accounts.authority,
            mint: &ctx.accounts.mint,
            system_program: &ctx.accounts.system_program,
            token_program: &ctx.accounts.token_program,
        };

        if ctx.signer_seeds.is_empty() {
            ix.invoke()
        } else {
            // Pure Zero-Alloc: Use stack arrays for seeds
            // SAFETY: We use core::mem::zeroed() to instantiate the stack arrays without initialization 
            // overhead. This is sound because both pinocchio::cpi::Signer and Seed are POD types where 
            // all-zero bit patterns are valid empty representations.
            let mut signers: [::pinocchio::cpi::Signer; 4] = unsafe { core::mem::zeroed() };
            let mut seeds_buffer: [::pinocchio::cpi::Seed; 32] = unsafe { core::mem::zeroed() };
            let mut seed_ranges = [(0usize, 0usize); 4]; 
            
            let mut seed_idx = 0;
            let mut signer_idx = 0;

            // Pass 1: Fill the seeds buffer
            for (i, seed_parts) in ctx.signer_seeds.iter().enumerate() {
                if i >= signers.len() { break; }
                let start_seed = seed_idx;
                for part in seed_parts.iter() {
                    if seed_idx >= seeds_buffer.len() { break; }
                    seeds_buffer[seed_idx] = ::pinocchio::cpi::Seed::from(*part);
                    seed_idx += 1;
                }
                seed_ranges[i] = (start_seed, seed_idx);
                signer_idx += 1;
            }

            // Pass 2: Create references from the frozen buffer
            for i in 0..signer_idx {
                let (start, end) = seed_ranges[i];
                signers[i] = ::pinocchio::cpi::Signer::from(&seeds_buffer[start..end]);
            }

            ix.invoke_signed(&signers[..signer_idx])
        }
    }
}