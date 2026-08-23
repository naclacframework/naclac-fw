use naclac_lang::prelude::*;
use crate::components::{BuybackVault, FeeProgramGlobal};
use crate::constants::{BUYBACK_VAULT_SEED, FEE_PROGRAM_GLOBAL_SEED};
use crate::errors::FeesError;

// naclac bans on-chain `find_program_address`, so `buyback_vault`'s seeds
// (dynamic on `index`) need an explicit, client-computed bump — `BuybackVault`
// has no stored `bump` field (fees-02), so this can't be read back either.
#[derive(Accounts)]
#[instruction(index: u8, buyback_vault_bump: u8)]
pub struct UpdateBuybackClaimRateLimit {
    #[account(mut)]
    pub authority: Signer,
    #[account(mut, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
    #[account(mut, seeds = [BUYBACK_VAULT_SEED, &[index]], bump = buyback_vault_bump)]
    pub buyback_vault: Account<BuybackVault>,
}

/// Gated by the *global* `fee_program_global.authority`, not the vault's own
/// `authority` — an intentional admin-override design (fees-05).
#[instruction]
pub fn update_buyback_claim_rate_limit(
    ctx: Context<UpdateBuybackClaimRateLimit>,
    _index: u8,
    _buyback_vault_bump: u8,
    claim_rate_limit: i64,
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.fee_program_global.authority,
        FeesError::InvalidAdmin
    );

    ctx.accounts.buyback_vault.claim_rate_limit = claim_rate_limit;

    Ok(())
}
