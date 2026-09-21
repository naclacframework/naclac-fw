use naclac_lang::prelude::*;

/// Reads `TransferHook` off a mint and `TransferHookAccount` off a token
/// account via `get_extension` (`naclac-token/src/extensions.rs`) and
/// asserts every field matches the expected values passed in — proves the
/// TLV byte-offset math for these two extension types end-to-end.
#[derive(Accounts)]
pub struct CheckTransferHook {
    pub mint: InterfaceAccount<Mint>,
    pub token_account: InterfaceAccount<TokenAccount>,
}

pub fn check_transfer_hook(
    ctx: Context<CheckTransferHook>,
    expected_authority: Option<Address>,
    expected_program_id: Option<Address>,
    expected_transferring: u8,
) -> Result {
    let hook: TransferHook = ctx.accounts.mint.get_extension()?;
    let hook_account: TransferHookAccount = ctx.accounts.token_account.get_extension()?;

    let matches = hook.authority() == expected_authority
        && hook.program_id() == expected_program_id
        && hook_account.transferring() == (expected_transferring != 0);

    if !matches {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
