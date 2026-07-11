// ===========================================================================
// associated_token.rs — Associated Token Program CPI helpers
// ===========================================================================

//! # Associated Token Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking the SPL Associated Token Program.
//! Enforces borrow-checking via CPI handles.

use crate::prelude::{CpiHandle, CpiHandleMut, Result};
use crate::wrappers::{ToCpiHandle, ToCpiHandleMut};

/// Accounts required for creating an associated token account.
pub struct Create<'a> {
    pub payer: CpiHandleMut<'a>,
    pub associated_token: CpiHandleMut<'a>,
    pub authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
    pub token_program: CpiHandle<'a>,
}

pub fn create(accounts: Create<'_>, _program: CpiHandle<'_>) -> Result<()> {
    create_signed(accounts, _program, &[])
}

pub fn create_signed(
    accounts: Create<'_>,
    _program: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            accounts.payer.info.key,
            accounts.authority.info.key,
            accounts.mint.info.key,
            accounts.token_program.info.key,
        );

        let accounts_array = [
            CpiHandle::from(accounts.payer),
            CpiHandle::from(accounts.associated_token),
            accounts.authority,
            accounts.mint,
            accounts.system_program,
            accounts.token_program,
            _program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts_array, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        use pinocchio_associated_token_account::instructions::Create as PinocchioCreate;

        let ix = PinocchioCreate {
            funding_account: &accounts.payer.info.view,
            account: &accounts.associated_token.info.view,
            wallet: &accounts.authority.info.view,
            mint: &accounts.mint.info.view,
            system_program: &accounts.system_program.info.view,
            token_program: &accounts.token_program.info.view,
        };

        if signer_seeds.is_empty() {
            ix.invoke()
        } else {
            let mut signers: [::pinocchio::cpi::Signer; 4] = unsafe { core::mem::zeroed() };
            let mut seeds_buffer: [::pinocchio::cpi::Seed; 32] = unsafe { core::mem::zeroed() };
            let mut seed_ranges = [(0usize, 0usize); 4];

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

            ix.invoke_signed(&signers[..signer_idx])
        }
    }
}

#[derive(Debug)]
pub struct CreateAtaAccounts<'a, A, B, C, D, E, F> {
    pub payer: &'a mut A,
    pub associated_token: &'a mut B,
    pub authority: &'a C,
    pub mint: &'a D,
    pub system_program: &'a E,
    pub token_program: &'a F,
}

pub trait AssociatedTokenCpi {
    fn create<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;

    fn create_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;
}

impl AssociatedTokenCpi for crate::wrappers::Program<crate::wrappers::AssociatedToken> {
    fn create<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let accounts_internal = Create {
            payer: accounts.payer.to_cpi_handle_mut(),
            associated_token: accounts.associated_token.to_cpi_handle_mut(),
            authority: accounts.authority.to_cpi_handle(),
            mint: accounts.mint.to_cpi_handle(),
            system_program: accounts.system_program.to_cpi_handle(),
            token_program: accounts.token_program.to_cpi_handle(),
        };
        let program_h = self.to_cpi_handle();
        create(accounts_internal, program_h)
    }

    fn create_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let accounts_internal = Create {
            payer: accounts.payer.to_cpi_handle_mut(),
            associated_token: accounts.associated_token.to_cpi_handle_mut(),
            authority: accounts.authority.to_cpi_handle(),
            mint: accounts.mint.to_cpi_handle(),
            system_program: accounts.system_program.to_cpi_handle(),
            token_program: accounts.token_program.to_cpi_handle(),
        };
        let program_h = self.to_cpi_handle();
        create_signed(accounts_internal, program_h, signer_seeds)
    }
}
