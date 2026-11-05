//! AMM pool state account layout.

use naclac_lang::prelude::*;

#[account]
pub struct PoolState {
    pub id: u64,
    pub token_a_mint: Address,
    pub token_b_mint: Address,
    pub vault_a: Address,
    pub vault_b: Address,
    pub lp_mint: Address,
    pub bump: u8,
}
