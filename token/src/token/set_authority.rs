//! `SetAuthority`: changes or clears one authority of a mint or token
//! account, signed by the current authority.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, SET_AUTHORITY},
    crate::prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result},
};

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
        let (data, len) = instruction_data(authority_type, new_authority);
        invoke(
            program,
            [
                (CpiHandle::from(account_or_mint), Role::Writable),
                (current_authority, Role::ReadonlySigner),
            ],
            &data[..len],
            signer_seeds,
        )
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
                data[0] = SET_AUTHORITY;
                data[1] = authority_type as u8;
                data[2] = 1;
                data[3..].copy_from_slice(new_authority);
                invoke(&program, &accounts, &handles, &data, signer_seeds)
            }
            None => invoke(
                &program,
                &accounts,
                &handles,
                &[SET_AUTHORITY, authority_type as u8, 0],
                signer_seeds,
            ),
        }
    }
}

/// The instruction data and how many of its 35 bytes are used: 3 when the
/// authority is cleared, 35 when it is set to a new address.
#[cfg(not(feature = "pinocchio"))]
fn instruction_data(
    authority_type: AuthorityType,
    new_authority: Option<&Address>,
) -> ([u8; 35], usize) {
    let mut data = [0u8; 35];
    data[0] = SET_AUTHORITY;
    data[1] = authority_type as u8;
    match new_authority {
        Some(new_authority) => {
            let new_authority: &[u8] = new_authority.as_ref();
            data[2] = 1;
            data[3..].copy_from_slice(new_authority);
            (data, 35)
        }
        None => (data, 3),
    }
}

#[cfg(all(test, not(feature = "pinocchio")))]
mod tests {
    use {
        super::*,
        spl_token_2022_interface::{instruction as token_2022, ID as TOKEN_2022_ID},
        spl_token_interface::{instruction as classic, ID as CLASSIC_ID},
    };

    const TYPES: [AuthorityType; 4] = [
        AuthorityType::MintTokens,
        AuthorityType::FreezeAccount,
        AuthorityType::AccountOwner,
        AuthorityType::CloseAccount,
    ];

    fn classic_type(authority_type: AuthorityType) -> classic::AuthorityType {
        match authority_type {
            AuthorityType::MintTokens => classic::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => classic::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => classic::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => classic::AuthorityType::CloseAccount,
        }
    }

    fn token_2022_type(authority_type: AuthorityType) -> token_2022::AuthorityType {
        match authority_type {
            AuthorityType::MintTokens => token_2022::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => token_2022::AuthorityType::FreezeAccount,
            AuthorityType::AccountOwner => token_2022::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => token_2022::AuthorityType::CloseAccount,
        }
    }

    #[test]
    fn the_data_is_what_both_token_programs_build() {
        let account = Address::new_from_array([1; 32]);
        let authority = Address::new_from_array([2; 32]);
        let new_authority = Address::new_from_array([0xab; 32]);
        for authority_type in TYPES {
            for new in [None, Some(&new_authority)] {
                let (data, len) = instruction_data(authority_type, new);
                let classic_data = classic::set_authority(
                    &CLASSIC_ID,
                    &account,
                    new,
                    classic_type(authority_type),
                    &authority,
                    &[],
                )
                .unwrap()
                .data;
                let token_2022_data = token_2022::set_authority(
                    &TOKEN_2022_ID,
                    &account,
                    new,
                    token_2022_type(authority_type),
                    &authority,
                    &[],
                )
                .unwrap()
                .data;
                assert!(data[..len] == classic_data[..]);
                assert!(data[..len] == token_2022_data[..]);
            }
        }
    }
}
