use naclac_lang::prelude::*;

/// Real end-to-end proof of `initialize_immutable_owner`
/// (`naclac-token/src/extensions/immutable_owner.rs`): allocates a plain
/// (non-PDA) token account by hand, since `InitializeImmutableOwner` must
/// run before `InitializeAccount3` — there is no auto-`init` path that
/// leaves room to inject the extension-init CPI in between. `token_account`
/// is a real transaction signer (not a PDA), so `system_program::create_account`
/// (the unsigned form) is sufficient — no seeds needed for it to satisfy the
/// System program's own signer requirement on the new account.
#[derive(Accounts)]
pub struct CreateTokenAccountWithImmutableOwner {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account` then two CPIs) — there is no naclac
    /// `Discriminator` to check since this is a raw SPL `TokenAccount` +
    /// `ImmutableOwner` extension layout, not a naclac component.
    #[account(mut)]
    pub token_account: Signer,

    pub mint: InterfaceAccount<Mint>,

    /// SAFETY: only used as an address — its own key becomes the new
    /// account's owner via `initialize_account` below; its data is never
    /// read or deserialized.
    pub owner: AccountInfo,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 165 bytes + 1-byte `AccountType` marker + 4-byte TLV header + 0-byte
/// `ImmutableOwner` value.
const TOKEN_ACCOUNT_WITH_IMMUTABLE_OWNER_SPACE: u64 = 165 + 1 + 4;

pub fn create_token_account_with_immutable_owner(
    ctx: Context<CreateTokenAccountWithImmutableOwner>,
) -> Result {
    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(TOKEN_ACCOUNT_WITH_IMMUTABLE_OWNER_SPACE as usize)?;

    system_program::create_account(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        TOKEN_ACCOUNT_WITH_IMMUTABLE_OWNER_SPACE,
        &ctx.accounts.token_program.address(),
    )?;

    naclac_lang::prelude::initialize_immutable_owner(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
    )?;

    naclac_lang::prelude::initialize_account(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.owner.to_cpi_handle(),
    )?;

    Ok(())
}
