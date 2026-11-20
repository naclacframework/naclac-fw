//! `InitializeMint2`: initializes a mint account that is already allocated
//! and owned by the token program.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, INITIALIZE_MINT2},
    crate::prelude::{Address, CpiHandle, CpiHandleMut, CpiSigner, Result},
};

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
    let data = instruction_data(decimals, mint_authority, freeze_authority);

    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [(CpiHandle::from(mint), Role::Writable)],
            &data,
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let mint = CpiHandle::from(mint);
        let accounts = [InstructionAccount::writable(mint.info.view.address())];
        invoke(&program, &accounts, &[mint], &data, &[])
    }
}

/// `[INITIALIZE_MINT2, decimals, mint_authority, freeze tag, freeze_authority]`.
/// With no freeze authority the tag and the 32 bytes after it are zero.
fn instruction_data(
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> [u8; 67] {
    let mut data = [0u8; 67];
    data[0] = INITIALIZE_MINT2;
    data[1] = decimals;
    let mint_authority: &[u8] = mint_authority.as_ref();
    data[2..34].copy_from_slice(mint_authority);
    if let Some(freeze) = freeze_authority {
        let freeze: &[u8] = freeze.as_ref();
        data[34] = 1;
        data[35..].copy_from_slice(freeze);
    }
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
    fn the_instruction_data_starts_with_what_both_token_programs_build() {
        let key = |seed: u8| Address::new_from_array([seed; 32]);
        let (mint, authority, freeze) = (key(1), key(2), key(0xAB));
        for freeze_authority in [None, Some(&freeze)] {
            let ours = instruction_data(6, &authority, freeze_authority);
            let classic_data =
                classic::initialize_mint2(&CLASSIC_ID, &mint, &authority, freeze_authority, 6)
                    .unwrap()
                    .data;
            let token_2022_data = token_2022::initialize_mint2(
                &TOKEN_2022_ID,
                &mint,
                &authority,
                freeze_authority,
                6,
            )
            .unwrap()
            .data;
            for built in [classic_data, token_2022_data] {
                assert!(ours[..built.len()] == built[..]);
                assert!(ours[built.len()..].iter().all(|byte| *byte == 0));
            }
        }
    }
}
