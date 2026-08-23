use naclac_lang::prelude::*;

/// Real `SetAuthority(AccountOwner)` CPI against a Token-2022 token account
/// (`naclac-token/src/token.rs`'s `set_authority_signed`, via
/// `Program<Token2022>`) — used both as the baseline (a normal vault's owner
/// can be reassigned) and, against a vault carrying `ImmutableOwner`, as the
/// real end-to-end proof that the extension actually blocks the change
/// on-chain rather than merely being readable.
#[derive(Accounts)]
pub struct SetTokenAccountOwner {
    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub current_owner: Signer,

    /// SAFETY: only used as an address — its own key becomes the vault's new
    /// owner via `set_authority_signed` below; its data is never read or
    /// deserialized.
    pub new_owner: AccountInfo,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn set_token_account_owner(ctx: Context<SetTokenAccountOwner>) -> Result {
    let new_owner_address = ctx.accounts.new_owner.address();

    ctx.accounts.token_program.set_authority_signed(
        SetAuthorityAccounts {
            account_or_mint: &mut ctx.accounts.vault,
            current_authority: &ctx.accounts.current_owner,
        },
        AuthorityType::AccountOwner,
        Some(&new_owner_address),
        &[],
    )?;

    Ok(())
}
