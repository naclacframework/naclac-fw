use naclac_lang::prelude::*;

#[instruction_args]
pub struct CheckAccountFieldsArgs {
    pub expected_vault_delegate: Option<Address>,
    pub expected_vault_delegated_amount: u64,
    pub expected_vault_state: u8,
    pub expected_vault_is_native: Option<u64>,
    pub expected_vault_close_authority: Option<Address>,
    pub expected_mint_authority: Option<Address>,
    pub expected_mint_freeze_authority: Option<Address>,
    pub expected_mint_is_initialized: u8,
}

/// Reads every base `TokenAccount`/`Mint` field naclac-token exposes an
/// accessor for — including the ones added alongside Tier 3
/// (`delegate`/`state`/`is_frozen`/`is_native`/`delegated_amount`/
/// `close_authority` on `TokenAccount`; `mint_authority`/`freeze_authority`/
/// `is_initialized` on `Mint`) — and asserts each against the expected
/// value passed in. The base-field equivalent of `check_transfer_fee_config`/
/// `check_transfer_hook` for the extension fields.
#[derive(Accounts)]
#[instruction(args: CheckAccountFieldsArgs)]
pub struct CheckAccountFields {
    pub vault: Account<TokenAccount>,
    pub mint: Account<Mint>,
}

pub fn check_account_fields(ctx: Context<CheckAccountFields>, args: CheckAccountFieldsArgs) -> Result {
    let vault = &ctx.accounts.vault;
    let mint = &ctx.accounts.mint;

    let matches = vault.delegate() == args.expected_vault_delegate
        && vault.delegated_amount() == args.expected_vault_delegated_amount
        && vault.state() == args.expected_vault_state
        && vault.is_frozen() == (args.expected_vault_state == 2)
        && vault.is_native() == args.expected_vault_is_native
        && vault.close_authority() == args.expected_vault_close_authority
        && mint.mint_authority() == args.expected_mint_authority
        && mint.freeze_authority() == args.expected_mint_freeze_authority
        && mint.is_initialized() == (args.expected_mint_is_initialized != 0);

    if !matches {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
