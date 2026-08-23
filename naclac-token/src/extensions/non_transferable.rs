// ===========================================================================
// extensions/non_transferable.rs — NonTransferable / NonTransferableAccount
// ===========================================================================

//! `NonTransferable` (mint extension) / `NonTransferableAccount`
//! (token-account extension) — both zero-byte presence markers, plus
//! `initialize_non_transferable_mint`, the CPI that actually sets
//! `NonTransferable` on a mint. An earlier pass wrongly assumed this
//! extension had no dedicated init CPI (confused it with extensions that
//! are set via a `mint::*` builder field at `InitializeMint` time); the
//! real protocol does have a separate `InitializeNonTransferableMint`
//! instruction, confirmed against `spl-token-2022-interface`'s real
//! `TokenInstruction::InitializeNonTransferableMint`/
//! `initialize_non_transferable_mint`, cross-checked against
//! `pinocchio-token-2022`'s own `InitializeNonTransferableMint::invoke`,
//! and matching anchor-spl-v2's own `token_2022_extensions::non_transferable`
//! module (`non_transferable_mint_initialize`), the parity target.
//! `NonTransferableAccount` has no CPI of its own — Token-2022 attaches it
//! automatically to every token account of a `NonTransferable` mint.

use super::Extension;
use crate::prelude::{CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `NonTransferable` mint extension — a zero-byte marker;
/// its mere presence (not any field value) means tokens of this mint can
/// never be transferred, only minted/burned. Layout verified against
/// `spl-token-2022-interface::extension::non_transferable::NonTransferable`,
/// a real zero-sized `#[repr(transparent)]` type there too, not
/// naclac-invented.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct NonTransferable;

unsafe impl Pod for NonTransferable {}
unsafe impl Zeroable for NonTransferable {}
impl Extension for NonTransferable {
    const TYPE: u16 = 9; // ExtensionType::NonTransferable
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

/// Raw Token-2022 `NonTransferableAccount` token-account extension — a
/// zero-byte marker Token-2022 attaches to every account of a
/// `NonTransferable` mint. Layout verified against
/// `spl-token-2022-interface::extension::non_transferable::NonTransferableAccount`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct NonTransferableAccount;

unsafe impl Pod for NonTransferableAccount {}
unsafe impl Zeroable for NonTransferableAccount {}
impl Extension for NonTransferableAccount {
    const TYPE: u16 = 13; // ExtensionType::NonTransferableAccount
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

/// Initializes the `NonTransferable` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as every other extension initializer. Instruction encoding
/// (top-level discriminant `32` = `InitializeNonTransferableMint`, no
/// sub-discriminant and no data payload) verified against
/// `spl-token-2022-interface`'s real
/// `TokenInstruction::InitializeNonTransferableMint`/
/// `initialize_non_transferable_mint`, cross-checked against
/// `pinocchio-token-2022`'s own `InitializeNonTransferableMint::invoke`.
pub fn initialize_non_transferable_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
) -> Result<()> {
    initialize_non_transferable_mint_signed(program, mint, &[])
}

pub fn initialize_non_transferable_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
                mint.info.address(),
                false,
            )],
            data: vec![32u8],
        };
        let accounts = [CpiHandle::from(mint), program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::InitializeNonTransferableMint {
            mint: &mint.info.view,
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}
