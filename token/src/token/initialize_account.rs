//! `InitializeAccount3`: initializes a token account that is already
//! allocated and owned by the token program.

#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};
#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
use crate::{
    prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result},
    wrappers::ToAddress,
};

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
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    let authority = ToAddress::address(&authority.info);
    initialize_account_with_owner_signed(program, account, mint, &authority, signer_seeds)
}

/// `InitializeAccount3`: the token account's owner is only written into the
/// instruction data, so it is given as an address, not an account.
pub fn initialize_account_with_owner_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    owner: &Address,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = [0u8; 33];
        data[0] = 18;
        data[1..].copy_from_slice(owner.as_ref());

        let ix = solana_instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_instruction::AccountMeta::new(account.info.address(), false),
                solana_instruction::AccountMeta::new_readonly(mint.info.address(), false),
            ],
            data: data.to_vec(),
        };
        let accounts = [CpiHandle::from(account), mint, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let account = CpiHandle::from(account);
        let accounts = [
            InstructionAccount::writable(account.info.view.address()),
            InstructionAccount::readonly(mint.info.view.address()),
        ];
        let mut data = [0u8; 33];
        data[0] = 18;
        let owner: &[u8] = owner.as_ref();
        data[1..].copy_from_slice(owner);
        invoke(&program, &accounts, &[account, mint], &data, &[])
    }
}
