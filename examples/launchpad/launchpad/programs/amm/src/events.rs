use naclac_lang::prelude::*;

#[event]
pub struct PoolInitialized {
    pub token_a_mint: Address,
    pub token_b_mint: Address,
    pub lp_amount: u64,
}

#[event]
pub struct SwapExecuted {
    pub user: Address,
    pub amount_in: u64,
    pub amount_out: u64,
}

#[event]
pub struct LiquidityAdded {
    pub token_a_mint: Address,
    pub token_b_mint: Address,
    pub amount_a: u64,
    pub amount_b: u64,
    pub lp_minted: u64,
}

#[event]
pub struct LiquidityRemoved {
    pub token_a_mint: Address,
    pub token_b_mint: Address,
    pub amount_a: u64,
    pub amount_b: u64,
    pub lp_burned: u64,
}
