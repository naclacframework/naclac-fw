//! `InitializeAccount3`: initializes a token account that is already
//! allocated and owned by the token program.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, INITIALIZE_ACCOUNT3},
    crate::{
        prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result},
        wrappers::ToAddress,
    },
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
    let data = instruction_data(owner);

    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(account), Role::Writable),
                (mint, Role::Readonly),
            ],
            &data,
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let account = CpiHandle::from(account);
        let accounts = [
            InstructionAccount::writable(account.info.view.address()),
            InstructionAccount::readonly(mint.info.view.address()),
        ];
        invoke(&program, &accounts, &[account, mint], &data, &[])
    }
}

/// `[INITIALIZE_ACCOUNT3, owner]`.
fn instruction_data(owner: &Address) -> [u8; 33] {
    let mut data = [0u8; 33];
    data[0] = INITIALIZE_ACCOUNT3;
    let owner: &[u8] = owner.as_ref();
    data[1..].copy_from_slice(owner);
    data
}

#[cfg(all(test, not(feature = "pinocchio")))]
mod tests {
    use {
        super::*,
        spl_token_2022_interface::{instruction as token_2022, ID as TOKEN_2022_ID},
        spl_token_interface::{instruction as classic, ID as CLASSIC_ID},
    };

    #[test]
    fn the_instruction_data_is_what_both_token_programs_build() {
        let key = |seed: u8| Address::new_from_array([seed; 32]);
        let (account, mint, owner) = (key(1), key(2), key(0xAB));
        let ours = instruction_data(&owner);
        let classic_data = classic::initialize_account3(&CLASSIC_ID, &account, &mint, &owner)
            .unwrap()
            .data;
        let token_2022_data =
            token_2022::initialize_account3(&TOKEN_2022_ID, &account, &mint, &owner)
                .unwrap()
                .data;
        assert!(ours[..] == classic_data[..]);
        assert!(ours[..] == token_2022_data[..]);
    }
}
