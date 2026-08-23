// ===========================================================================
// extensions/group_pointer.rs — GroupPointer
// ===========================================================================

//! `GroupPointer` (mint extension), plus every CPI naclac-token supports
//! against it: `initialize_group_pointer`/`update_group_pointer` —
//! confirmed against anchor-spl-v2's own
//! `token_2022_extensions::group_pointer` module as the parity target,
//! which exposes exactly these two functions.

use super::{read_optional_address, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `GroupPointer` mint extension (64 bytes): the authority
/// allowed to change the group address, plus the account that holds the
/// group configuration itself (which may be the mint account or a separate
/// account). Layout verified against
/// `spl-token-2022-interface::extension::group_pointer::GroupPointer`'s
/// real field order/sizes.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct GroupPointer(pub [u8; 64]);

unsafe impl Pod for GroupPointer {}
unsafe impl Zeroable for GroupPointer {}
impl Extension for GroupPointer {
    const TYPE: u16 = 20; // ExtensionType::GroupPointer
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl GroupPointer {
    pub fn authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
    pub fn group_address(&self) -> Option<Address> {
        read_optional_address(&self.0, 32)
    }
}

/// Initializes the `GroupPointer` extension on a not-yet-initialized mint.
/// Must be called before `initialize_mint`/`initialize_mint_signed`.
/// Instruction encoding (top-level discriminant `40` =
/// `GroupPointerExtension`, sub-discriminant `0` = `Initialize`, then
/// `authority`/`group_address` each as a fixed 32 bytes, all-zero meaning
/// `None`) verified against `spl-token-2022-interface`'s real
/// `group_pointer::instruction::initialize`, cross-checked against
/// `pinocchio-token-2022`'s own `extensions::group_pointer::Initialize::invoke`.
pub fn initialize_group_pointer(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    group_address: Option<&Address>,
) -> Result<()> {
    initialize_group_pointer_signed(program, mint, authority, group_address, &[])
}

pub fn initialize_group_pointer_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    group_address: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![40u8, 0u8];
        data.extend_from_slice(authority.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));
        data.extend_from_slice(group_address.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

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
        let ix = ::pinocchio_token_2022::instructions::group_pointer::Initialize {
            mint: &mint.info.view,
            authority: authority.map(|a| a.as_address()),
            group_address: group_address.map(|a| a.as_address()),
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Updates an already-initialized mint's group pointer address. Signed by
/// the mint's group-pointer authority. Instruction encoding (top-level
/// discriminant `40`, sub-discriminant `1` = `Update`, then `group_address`
/// as a fixed 32 bytes, all-zero meaning `None`) verified against
/// `spl-token-2022-interface`'s real
/// `GroupPointerInstruction::Update`/`update`, cross-checked against
/// `pinocchio-token-2022`'s own `extensions::group_pointer::Update::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Update` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn update_group_pointer(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    group_address: Option<&Address>,
) -> Result<()> {
    update_group_pointer_signed(program, mint, authority, group_address, &[])
}

pub fn update_group_pointer_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    group_address: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![40u8, 1u8];
        data.extend_from_slice(group_address.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [CpiHandle::from(mint), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 34];
        data[0] = 40;
        data[1] = 1;
        data[2..34].copy_from_slice(group_address.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [mint_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
