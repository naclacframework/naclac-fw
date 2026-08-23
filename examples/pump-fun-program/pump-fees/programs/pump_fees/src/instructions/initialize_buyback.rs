use naclac_lang::prelude::*;
use crate::components::BuybackVault;
use crate::constants::{BUYBACK_VAULT_SEED, MAX_BUYBACK_INDEX};
use crate::errors::FeesError;

// naclac bans on-chain `find_program_address`, so `buyback_vault`'s seeds
// (dynamic on `index`) need an explicit, client-computed bump — the real
// instruction takes only `index` since Anchor derives it on-chain itself.
#[derive(Accounts)]
#[instruction(index: u8, buyback_vault_bump: u8)]
pub struct InitializeBuyback {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [BUYBACK_VAULT_SEED, &[index]], bump = buyback_vault_bump)]
    pub buyback_vault: Account<BuybackVault>,
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = buyback_vault)]
    pub buyback_vault_ata: Account<TokenAccount>,
    pub system_program: Program<System>,
    pub associated_token_program: Program<AssociatedToken>,
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
}

// fees-06 #5: real on-chain behavior — `authority` (and every other field)
// is left at its zero-initialized default, not set to `payer`. A freshly
// `init`'d account is already all-zero, so there's nothing to write here.
#[instruction]
pub fn initialize_buyback(
    _ctx: Context<InitializeBuyback>,
    index: u8,
    _buyback_vault_bump: u8,
) -> Result {
    require!(index < MAX_BUYBACK_INDEX, FeesError::InvalidBuybackIndex);
    Ok(())
}
