// ===========================================================================
// system_program.rs — Dual-backend system program helpers
// ===========================================================================

//! # System Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking native Solana System Program instructions.
//! In particular, this abstracts away the complex zero-allocation seed generation required 
//! by `pinocchio_system` when performing PDA-signed transfers.

// SOLANA BACKEND
#[cfg(not(feature = "pinocchio"))]
mod solana_system {
    use crate::cpi::CpiContext;
    use solana_program::program::invoke_signed;
    use crate::prelude::Result;

    /// The Solana System Program ID (all zeros)
    pub const ID: solana_program::pubkey::Pubkey =
        solana_program::pubkey::Pubkey::new_from_array([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);

    pub struct Transfer<'info> {
        pub from: solana_program::account_info::AccountInfo<'info>,
        pub to: solana_program::account_info::AccountInfo<'info>,
    }

    pub fn transfer<'a, 'b, 'c, 'info>(
        ctx: CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>,
        lamports: u64,
    ) -> Result<()> {
        let ix = solana_system_interface::instruction::transfer(
            ctx.accounts.from.key,
            ctx.accounts.to.key,
            lamports,
        );
        invoke_signed(
            &ix,
            &[ctx.accounts.from.clone(), ctx.accounts.to.clone()],
            ctx.signer_seeds,
        )
    }
}

// PINOCCHIO BACKEND
#[cfg(feature = "pinocchio")]
mod pinocchio_system {
    use crate::cpi::CpiContext;
    use crate::prelude::Result;

    // Re-use the system program ID directly from pinocchio-system crate
    // to avoid any type construction issues.
    pub use ::pinocchio_system::ID;

    pub struct Transfer<'info> {
        pub from: crate::prelude::AccountInfo<'info>,
        pub to: crate::prelude::AccountInfo<'info>,
    }

    pub fn transfer<'a, 'b, 'c, 'info>(
        ctx: CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>,
        lamports: u64,
    ) -> Result<()> {
        let ix = ::pinocchio_system::instructions::Transfer {
            from: &ctx.accounts.from.view,
            to: &ctx.accounts.to.view,
            lamports,
        };

        // For non-PDA transfers (no signer seeds) use invoke()
        // For PDA transfers, invoke_signed requires pinocchio::cpi::Signer
        // which is built from &[Seed] — handled by the caller building the CpiContext.
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

// ===========================================================================
// Re-exports
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_system::{ID, Transfer, transfer};

#[cfg(feature = "pinocchio")]
pub use pinocchio_system::{ID, Transfer, transfer};