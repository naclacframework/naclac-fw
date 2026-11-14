//! `InitializeMint2`: initializes a mint account that is already allocated
//! and owned by the token program.

use crate::prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::{prelude::vec, wrappers::ToAddress};
#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};

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
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![20u8];
        data.push(decimals);
        data.extend_from_slice(mint_authority.as_ref());
        if let Some(freeze) = freeze_authority {
            data.push(1);
            data.extend_from_slice(freeze.as_ref());
        } else {
            data.push(0);
            data.extend_from_slice(&[0u8; 32]);
        }

        let ix = solana_instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_instruction::AccountMeta::new(
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
        let mint = CpiHandle::from(mint);
        let accounts = [InstructionAccount::writable(mint.info.view.address())];
        let mut data = [0u8; 67];
        data[0] = 20;
        data[1] = decimals;
        let mint_authority: &[u8] = mint_authority.as_ref();
        data[2..34].copy_from_slice(mint_authority);
        if let Some(freeze) = freeze_authority {
            let freeze: &[u8] = freeze.as_ref();
            data[34] = 1;
            data[35..].copy_from_slice(freeze);
        }
        invoke(&program, &accounts, &[mint], &data, &[])
    }
}
