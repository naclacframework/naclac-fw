use naclac_lang::prelude::*;
use crate::state::MintAuthority;

// `token::mint`/`token::authority` on an *existing* (non-`init`) token
// account — mirrors `examples/escrow/.../make.rs`'s `vault_token_account`
// exactly. Reads bytes `[0..32]`/`[32..64]` of the vault's raw SPL layout;
// a mint or authority mismatch is `ConstraintAddress`.
#[derive(Accounts)]
pub struct CheckVaultConstraints {
    pub mint: Account<Mint>,
    pub mint_authority: Account<MintAuthority>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = mint_authority,
    )]
    pub vault: Account<TokenAccount>,
}

pub fn check_vault_constraints(_ctx: Context<CheckVaultConstraints>) -> Result {
    Ok(())
}
