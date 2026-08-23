// ===========================================================================
// extensions/immutable_owner.rs — ImmutableOwner
// ===========================================================================

//! `ImmutableOwner` (token-account extension), plus its only CPI —
//! `initialize_immutable_owner`. Confirmed at full parity with
//! anchor-spl-v2's own `token_2022_extensions::immutable_owner` module,
//! which exposes nothing beyond its own initializer either: once set, the
//! extension is enforced by the protocol itself on every `SetAuthority`
//! targeting the account's owner, no dedicated action CPI exists.

use super::Extension;
use crate::prelude::{CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `ImmutableOwner` token-account extension — a zero-byte
/// marker; its mere presence means the account's owner can never be
/// reassigned via `SetAuthority`. Layout verified against
/// `spl-token-2022-interface::extension::immutable_owner::ImmutableOwner`,
/// a real zero-sized `#[repr(transparent)]` type there too, not
/// naclac-invented.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct ImmutableOwner;

unsafe impl Pod for ImmutableOwner {}
unsafe impl Zeroable for ImmutableOwner {}
impl Extension for ImmutableOwner {
    const TYPE: u16 = 7; // ExtensionType::ImmutableOwner
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

/// Initializes the `ImmutableOwner` extension on a not-yet-initialized
/// token account. Must be called before `initialize_account`/
/// `initialize_account_signed` — Token-2022 rejects this on an
/// already-initialized account. Instruction encoding (top-level
/// discriminant `22` = `InitializeImmutableOwner`, no sub-discriminant and
/// no data payload — unlike every mint-extension initializer, this is a
/// direct `TokenInstruction` variant, not an `*Extension` wrapper, since
/// `ImmutableOwner` is set on the token account itself, not the mint)
/// verified against `spl-token-2022-interface`'s real
/// `TokenInstruction::InitializeImmutableOwner`/`initialize_immutable_owner`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `InitializeImmutableOwner::invoke`.
pub fn initialize_immutable_owner(program: CpiHandle<'_>, account: CpiHandleMut<'_>) -> Result<()> {
    initialize_immutable_owner_signed(program, account, &[])
}

pub fn initialize_immutable_owner_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
                account.info.address(),
                false,
            )],
            data: vec![22u8],
        };
        let accounts = [CpiHandle::from(account), program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::InitializeImmutableOwner {
            account: &account.info.view,
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}
