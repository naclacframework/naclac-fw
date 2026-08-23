use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `burn` CPI, signed by the `mint_authority` PDA's own seeds (the same
// PDA that owns every vault in this test case, per `create_token_account`'s
// `owner` argument at setup time) — mirrors `mint_to_vault.rs`'s signer-seeds
// construction exactly. `burn`/`burn_signed` (`naclac-token/src/token.rs`)
// had zero test coverage before this.
#[derive(Accounts)]
pub struct BurnVaultTokens {
    #[account(mut)]
    pub mint: Account<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

#[instruction]
pub fn burn_vault_tokens(ctx: Context<BurnVaultTokens>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.burn_signed(
        naclac_lang::prelude::BurnAccounts {
            mint: &mut ctx.accounts.mint,
            from: &mut ctx.accounts.vault,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        signer,
    )?;

    Ok(())
}
