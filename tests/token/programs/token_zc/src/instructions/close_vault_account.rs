use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `close_account`/`close_account_signed` CPI (`naclac-token/src/token.rs`)
// — the raw SPL Token instruction that closes a token account and returns
// its lamports, distinct from naclac's own `#[account(close = ...)]`
// framework constraint (already covered by
// `tests/accounts-constraints/`'s `close_vault`, which only ever closes a
// naclac-owned `#[component]` account, never a real SPL token account).
// Signed by the `mint_authority` PDA owner.
#[derive(Accounts)]
pub struct CloseVaultAccount {
    #[account(mut)]
    pub vault: Account<TokenAccount>,

    /// SAFETY: only used as the lamport-destination address for
    /// `close_account_signed` below; its data is never read or deserialized.
    #[account(mut)]
    pub destination: AccountInfo,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
}

#[instruction]
pub fn close_vault_account(ctx: Context<CloseVaultAccount>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.close_account_signed(
        naclac_lang::prelude::CloseAccountAccounts {
            account: &mut ctx.accounts.vault,
            destination: &mut ctx.accounts.destination,
            authority: &ctx.accounts.mint_authority,
        },
        signer,
    )?;

    Ok(())
}
