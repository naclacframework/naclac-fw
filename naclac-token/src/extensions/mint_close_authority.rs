// ===========================================================================
// extensions/mint_close_authority.rs — MintCloseAuthority
// ===========================================================================

//! `MintCloseAuthority` (mint extension), plus its only CPI —
//! `initialize_mint_close_authority`. Confirmed at full parity with both
//! the official `anchor-spl` crate and the local pinocchio-native
//! `anchor-spl-v2`'s own modules, which expose nothing beyond this
//! initializer either: the actual close, once this extension permits it on
//! a supply-zero mint, goes through the ordinary `close_account`/
//! `close_account_signed` CPI naclac-token already has — no dedicated
//! "close mint" action exists in the real protocol.

use super::Extension;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `MintCloseAuthority` mint extension (32 bytes): the
/// authority allowed to close the mint once its supply is zero. Layout
/// verified against
/// `spl-token-2022-interface::extension::mint_close_authority::MintCloseAuthority`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct MintCloseAuthority(pub [u8; 32]);

unsafe impl Pod for MintCloseAuthority {}
unsafe impl Zeroable for MintCloseAuthority {}
impl Extension for MintCloseAuthority {
    const TYPE: u16 = 3; // ExtensionType::MintCloseAuthority
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl MintCloseAuthority {
    pub fn close_authority(&self) -> Option<Address> {
        super::read_optional_address(&self.0, 0)
    }
}

/// Initializes the `MintCloseAuthority` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Instruction encoding is a
/// direct top-level `TokenInstruction::InitializeMintCloseAuthority`
/// variant (discriminant `25`, no `*Extension` sub-discriminant wrapper —
/// same shape as `InitializeImmutableOwner`, since this too is set on
/// account creation, not layered as a mint-extension-specific instruction
/// prefix), with a genuinely variable-length payload: a 1-byte presence tag
/// (`0` = `None`, `1` = `Some`) followed by 32 authority bytes only when
/// present — unlike every other extension initializer in this crate, which
/// writes a fixed 32 bytes with all-zero meaning `None`. Verified against
/// `spl-token-2022-interface`'s real
/// `TokenInstruction::InitializeMintCloseAuthority`/`pack_pubkey_option`/
/// `initialize_mint_close_authority`, cross-checked against
/// `pinocchio-token-2022`'s own
/// `InitializeMintCloseAuthority::invoke`, which encodes the identical
/// variable length.
pub fn initialize_mint_close_authority(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    close_authority: Option<&Address>,
) -> Result<()> {
    initialize_mint_close_authority_signed(program, mint, close_authority, &[])
}

pub fn initialize_mint_close_authority_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    close_authority: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![25u8];
        match close_authority {
            Some(authority) => {
                data.push(1);
                data.extend_from_slice(authority.as_ref());
            }
            None => data.push(0),
        }

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
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
        let ix = ::pinocchio_token_2022::instructions::mint_close_authority::InitializeMintCloseAuthority {
            mint: &mint.info.view,
            close_authority: close_authority.map(|a| a.as_address()),
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}
