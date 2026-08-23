// ===========================================================================
// extensions/permanent_delegate.rs — PermanentDelegate
// ===========================================================================

//! `PermanentDelegate` (mint extension), plus its only CPI —
//! `initialize_permanent_delegate`. Confirmed at full parity with
//! anchor-spl-v2's own `token_2022_extensions::permanent_delegate` module,
//! which exposes nothing beyond its own initializer either: the delegate
//! acts through the ordinary `transfer`/`burn` CPIs once set, no dedicated
//! action CPI exists in the real protocol.

use super::{read_optional_address, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `PermanentDelegate` mint extension (32 bytes): an optional
/// authority that can `Transfer`/`Burn` from *any* account holding this
/// mint's tokens, without needing that account's own approval — the mint's
/// declared permanent delegate, distinct from any per-account `Approve`
/// delegate. Layout verified against
/// `spl-token-2022-interface::extension::permanent_delegate::PermanentDelegate`'s
/// real field (a single `OptionalNonZeroPubkey`).
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PermanentDelegate(pub [u8; 32]);

unsafe impl Pod for PermanentDelegate {}
unsafe impl Zeroable for PermanentDelegate {}
impl Extension for PermanentDelegate {
    const TYPE: u16 = 12; // ExtensionType::PermanentDelegate
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl PermanentDelegate {
    pub fn delegate(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
}

/// Initializes the `PermanentDelegate` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed` —
/// Token-2022 rejects extension-initialize instructions on an already-
/// initialized mint. Unlike `TransferFeeConfig`'s/`TransferHook`'s
/// authorities, `delegate` is not optional here: the real
/// `InitializePermanentDelegate` instruction always takes a concrete pubkey,
/// verified against `spl-token-2022-interface`'s real
/// `TokenInstruction::InitializePermanentDelegate { delegate: Pubkey }`
/// (discriminant `35`, no presence tag, 32 raw bytes), and independently
/// cross-checked against `pinocchio-token-2022`'s own
/// `InitializePermanentDelegate::invoke` — both agree.
pub fn initialize_permanent_delegate(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    delegate: &Address,
) -> Result<()> {
    initialize_permanent_delegate_signed(program, mint, delegate, &[])
}

pub fn initialize_permanent_delegate_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    delegate: &Address,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![35u8];
        data.extend_from_slice(delegate.as_ref());

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
        let ix =
            ::pinocchio_token_2022::instructions::permanent_delegate::InitializePermanentDelegate {
                mint: &mint.info.view,
                delegate: delegate.as_address(),
                token_program: program.info.view.address(),
            };
        ix.invoke()
    }
}
