//! `SetAuthority`: changes or clears one authority of a mint or token
//! account, signed by the current authority.

#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum AuthorityType {
    MintTokens,
    FreezeAccount,
    AccountOwner,
    CloseAccount,
}

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
    signer_seeds: &[CpiSigner<'_, '_>],
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
        let account_or_mint = CpiHandle::from(account_or_mint);
        let accounts = [
            InstructionAccount::writable(account_or_mint.info.view.address()),
            InstructionAccount::readonly_signer(current_authority.info.view.address()),
        ];
        let handles = [account_or_mint, current_authority];
        match new_authority {
            Some(new_authority) => {
                let new_authority: &[u8] = new_authority.as_ref();
                let mut data = [0u8; 35];
                data[0] = 6;
                data[1] = authority_type as u8;
                data[2] = 1;
                data[3..].copy_from_slice(new_authority);
                invoke(&program, &accounts, &handles, &data, signer_seeds)
            }
            None => invoke(
                &program,
                &accounts,
                &handles,
                &[6, authority_type as u8, 0],
                signer_seeds,
            ),
        }
    }
}
