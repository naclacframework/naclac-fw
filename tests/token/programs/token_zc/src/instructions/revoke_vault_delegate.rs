use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `revoke`/`revoke_signed` CPI (`naclac-token/src/token.rs`), the
// counterpart to `approve_vault_delegate.rs` — signed by the same
// `mint_authority` PDA owner.
#[derive(Accounts)]
pub struct RevokeVaultDelegate {
    #[account(mut)]
    pub vault: Account<TokenAccount>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
}

pub fn revoke_vault_delegate(ctx: Context<RevokeVaultDelegate>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.revoke_signed(
        naclac_lang::prelude::RevokeAccounts {
            source: &mut ctx.accounts.vault,
            authority: &ctx.accounts.mint_authority,
        },
        signer,
    )?;

    Ok(())
}
