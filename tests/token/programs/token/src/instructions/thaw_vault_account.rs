use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `thaw_account`/`thaw_account_signed` CPI (`naclac-token/src/token.rs`),
// the counterpart to `freeze_vault_account.rs` — signed by the same
// `mint_authority` PDA freeze authority.
#[derive(Accounts)]
pub struct ThawVaultAccount {
    #[account(mut)]
    pub vault: Account<TokenAccount>,

    pub mint: Account<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
}

pub fn thaw_vault_account(ctx: Context<ThawVaultAccount>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.thaw_account_signed(
        naclac_lang::prelude::ThawAccountAccounts {
            account: &mut ctx.accounts.vault,
            mint: &ctx.accounts.mint,
            authority: &ctx.accounts.mint_authority,
        },
        signer,
    )?;

    Ok(())
}
