use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `mint_to` CPI, signed by the `mint_authority` PDA's own seeds —
// mirrors `examples/launchpad/.../launch_token.rs`'s `mint_to_signed` call
// exactly (same signer-seeds construction).
#[derive(Accounts)]
pub struct MintToVault {
    #[account(mut)]
    pub mint: Account<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

pub fn mint_to_vault(ctx: Context<MintToVault>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.mint_to_signed(
        naclac_lang::prelude::MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.vault,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        signer,
    )?;

    Ok(())
}
