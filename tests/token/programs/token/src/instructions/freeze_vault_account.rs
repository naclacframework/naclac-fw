use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `freeze_account`/`freeze_account_signed` CPI (`naclac-token/src/token.rs`),
// signed by the `mint_authority` PDA — which is also the mint's real freeze
// authority whenever the mint was created via `create_mint_with_freeze`.
// Previously the only thing in this test case that ever touched freezing was
// `check_mint_freeze_authority`'s *constraint* check, which never actually
// froze an account; this is the first test to call the real CPI.
#[derive(Accounts)]
pub struct FreezeVaultAccount {
    #[account(mut)]
    pub vault: Account<TokenAccount>,

    pub mint: Account<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
}

#[instruction]
pub fn freeze_vault_account(ctx: Context<FreezeVaultAccount>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.freeze_account_signed(
        naclac_lang::prelude::FreezeAccountAccounts {
            account: &mut ctx.accounts.vault,
            mint: &ctx.accounts.mint,
            authority: &ctx.accounts.mint_authority,
        },
        signer,
    )?;

    Ok(())
}
