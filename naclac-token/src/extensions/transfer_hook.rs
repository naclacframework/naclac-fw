// ===========================================================================
// extensions/transfer_hook.rs — TransferHook / TransferHookAccount
// ===========================================================================

//! `TransferHook` (mint extension) / `TransferHookAccount` (token-account
//! extension), plus every CPI naclac-token supports against them:
//! `initialize_transfer_hook`/`transfer_hook_update` — confirmed against
//! anchor-spl-v2's own `token_2022_extensions::transfer_hook` module as the
//! parity target, which exposes exactly these two functions and no
//! extra-account-resolution machinery of its own. `transfer_checked_with_hook`
//! (non-pinocchio only, see its own doc comment) goes beyond that parity
//! target: it resolves and appends a hook-gated mint's required extra
//! accounts and actually invokes the hook during transfer, using the real
//! `spl-transfer-hook-interface` crate's own on-chain resolution helper
//! rather than a naclac reimplementation of its PDA/seed logic.

use super::{read_optional_address, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `TransferHook` mint extension (64 bytes): the authority
/// allowed to change the hook program, and the hook program itself. Layout
/// verified against `spl-token-2022-interface::extension::transfer_hook::TransferHook`'s
/// real field order/sizes.
///
/// Reading this only tells a program which hook program (if any) is
/// configured — it does not by itself resolve or append the hook's required
/// "extra accounts" to a transfer CPI (`transfer_checked_with_hook`, below,
/// does that). Token-2022 itself enforces their presence at the protocol
/// level: a transfer against a hook-gated mint fails on-chain without them.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TransferHook(pub [u8; 64]);

unsafe impl Pod for TransferHook {}
unsafe impl Zeroable for TransferHook {}
impl Extension for TransferHook {
    const TYPE: u16 = 14; // ExtensionType::TransferHook
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl TransferHook {
    pub fn authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
    pub fn program_id(&self) -> Option<Address> {
        read_optional_address(&self.0, 32)
    }
}

/// Raw Token-2022 `TransferHookAccount` token-account extension (1 byte):
/// whether this account is currently mid-transfer — Token-2022 sets this
/// itself around a hook-gated transfer's CPI so the hook program (and
/// anything it calls back into) can detect reentrancy; it is not something
/// an application program sets.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TransferHookAccount(pub [u8; 1]);

unsafe impl Pod for TransferHookAccount {}
unsafe impl Zeroable for TransferHookAccount {}
impl Extension for TransferHookAccount {
    const TYPE: u16 = 15; // ExtensionType::TransferHookAccount
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

impl TransferHookAccount {
    pub fn transferring(&self) -> bool {
        self.0[0] != 0
    }
}

/// Initializes the `TransferHook` extension on a not-yet-initialized mint.
/// Must be called before `initialize_mint`/`initialize_mint_signed`, same as
/// `initialize_transfer_fee_config`. Instruction encoding (top-level
/// discriminant `36` = `TransferHookExtension`, sub-discriminant `0` =
/// `Initialize`, then `authority`/`program_id` each as a fixed 32 bytes,
/// all-zero meaning `None` — a genuinely different convention from
/// `TransferFeeConfig`'s 1-byte-tag encoding) verified against
/// `spl-token-2022-interface`'s real `transfer_hook::instruction::initialize`/
/// `InitializeInstructionData`, cross-checked against
/// `pinocchio-token-2022`'s own `InitializeTransferHook::invoke`.
pub fn initialize_transfer_hook(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    hook_program_id: Option<&Address>,
) -> Result<()> {
    initialize_transfer_hook_signed(program, mint, authority, hook_program_id, &[])
}

pub fn initialize_transfer_hook_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    hook_program_id: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![36u8, 0u8];
        data.extend_from_slice(authority.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));
        data.extend_from_slice(hook_program_id.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

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
        let ix = ::pinocchio_token_2022::instructions::transfer_hook::InitializeTransferHook {
            mint: &mint.info.view,
            authority: authority.map(|a| a.as_address()),
            program_id: hook_program_id.map(|a| a.as_address()),
            token_program: program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Updates an already-initialized mint's transfer hook program/authority.
/// Signed by the mint's transfer-hook authority. Instruction encoding
/// (top-level discriminant `36` = `TransferHookExtension`, sub-discriminant
/// `1` = `Update`, then `program_id` as a fixed 32 bytes, all-zero meaning
/// `None` — same convention as `Initialize`'s own fields) verified against
/// `spl-token-2022-interface`'s real
/// `transfer_hook::instruction::{TransferHookInstruction::Update, update}`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `UpdateTransferHook::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `UpdateTransferHook`
/// struct — it's generic over a multisig-signer type, and naclac-token
/// supports only a single fixed authority everywhere else (see
/// `transfer_fee.rs`'s own note on the same tradeoff for its ongoing-action
/// CPIs). The raw `InstructionView` is hand-built instead.
pub fn transfer_hook_update(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    hook_program_id: Option<&Address>,
) -> Result<()> {
    transfer_hook_update_signed(program, mint, authority, hook_program_id, &[])
}

pub fn transfer_hook_update_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    hook_program_id: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![36u8, 1u8];
        data.extend_from_slice(hook_program_id.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

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
        data[0] = 36;
        data[1] = 1;
        data[2..34].copy_from_slice(hook_program_id.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));

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

/// Real `TransferChecked` against a `TransferHook`-gated mint, including
/// resolving and appending the hook's required extra accounts and CPI-ing
/// into the hook program itself — the piece `initialize_transfer_hook`/
/// `transfer_hook_update` alone don't cover. Built directly on the real
/// `spl-transfer-hook-interface` crate's `onchain::add_extra_accounts_for_execute_cpi`
/// (the same on-chain resolution helper the whole ecosystem uses for this
/// exact scenario — a program CPI-ing a hook-gated transfer into
/// Token-2022) rather than a naclac reimplementation of its PDA/seed
/// resolution logic, since getting that byte-for-byte right by hand would
/// be exactly the kind of security-critical reimplementation this project's
/// rules warn against when a verified reference already exists.
///
/// `extra_accounts` must contain, in any order, every account the caller
/// received from Token-2022 as part of resolving this hook (typically via
/// `spl-transfer-hook-interface`'s own off-chain `add_extra_account_metas_for_execute`
/// on the client, or by re-deriving `get_extra_account_metas_address` and
/// fetching it): the hook program account itself, the hook's
/// `ExtraAccountMetaList` PDA, and any further accounts its seed configs
/// reference. These aren't naclac-declared accounts — `remaining_accounts`
/// is the documented mechanism (`naclac-client/src/builder.rs`) for exactly
/// this "token-2022 extensions... governance programs" case.
///
/// `solana`-backend only. `pinocchio-token-2022` has no equivalent official
/// helper crate for this resolution step (`spl-transfer-hook-interface`'s
/// `onchain` module is hardcoded to `solana_account_info::AccountInfo`, not
/// pinocchio's account types), and hand-rolling the seed/PDA resolution
/// logic against pinocchio types without an official reference to verify
/// against would be exactly the kind of unverified reimplementation this
/// project's rules warn against — left as an explicit, tracked gap rather
/// than attempted here.
#[cfg(not(feature = "pinocchio"))]
pub fn transfer_checked_with_hook(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    hook_program: CpiHandle<'_>,
    extra_accounts: &[CpiHandle<'_>],
    amount: u64,
    decimals: u8,
) -> Result<()> {
    transfer_checked_with_hook_signed(
        program,
        from,
        mint,
        to,
        authority,
        hook_program,
        extra_accounts,
        amount,
        decimals,
        &[],
    )
}

#[cfg(not(feature = "pinocchio"))]
pub fn transfer_checked_with_hook_signed(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    hook_program: CpiHandle<'_>,
    extra_accounts: &[CpiHandle<'_>],
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let mut cpi_instruction = ::spl_token_2022::instruction::transfer_checked(
        &program.address(),
        &from.info.address(),
        &mint.info.address(),
        &to.info.address(),
        &authority.info.address(),
        &[],
        amount,
        decimals,
    )?;

    // SAFETY: each `.info` is a live `CpiHandle`'s account info, borrowed for
    // the duration of this call only — matches the same lifetime-erasure
    // `naclac-core::cpi::invoke_signed` already performs internally for
    // every other CPI helper in this crate.
    let source_info = unsafe { from.info.to_lifetime() };
    let mint_info = unsafe { mint.info.to_lifetime() };
    let destination_info = unsafe { to.info.to_lifetime() };
    let authority_info = unsafe { authority.info.to_lifetime() };

    let mut cpi_account_infos = vec![
        source_info.clone(),
        mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
    ];

    let additional_account_infos: Vec<_> = extra_accounts
        .iter()
        .map(|handle| unsafe { handle.info.to_lifetime() })
        .collect();

    let hook_program_address = hook_program.address();

    ::spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi(
        &mut cpi_instruction,
        &mut cpi_account_infos,
        &hook_program_address,
        source_info,
        mint_info,
        destination_info,
        authority_info,
        amount,
        &additional_account_infos,
    )?;

    solana_program::program::invoke_signed(&cpi_instruction, &cpi_account_infos, signer_seeds)
}
