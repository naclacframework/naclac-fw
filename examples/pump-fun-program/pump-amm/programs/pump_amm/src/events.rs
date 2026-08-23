use naclac_lang::prelude::*;

/// Field-for-field mirror of the real `pump_amm::CreatePoolEvent` (confirmed
/// against `reference/pump-rust-client/idls/pump_amm.json`).
// Field order deliberately differs from the real IDL's own listing — grouped
// so every 8-byte-aligned field (`i64`/`u64`) lands on an aligned offset with
// no internal gaps for `derive(Pod)`, all the byte-sized fields pushed to the
// end where only trailing (not internal) padding can occur. Purely a wire-
// layout concern: this event is never read back on-chain, only logged for
// off-chain indexers, so its field order carries no cross-program dependency
// the way component layouts (e.g. `Pool`) do.
#[event]
pub struct CreatePoolEvent {
    pub timestamp: i64,
    pub creator: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub base_amount_in: u64,
    pub quote_amount_in: u64,
    pub pool_base_amount: u64,
    pub pool_quote_amount: u64,
    pub minimum_liquidity: u64,
    pub initial_liquidity: u64,
    pub lp_token_amount_out: u64,
    pub pool: Address,
    pub lp_mint: Address,
    pub user_base_token_account: Address,
    pub user_quote_token_account: Address,
    pub coin_creator: Address,
    pub index: u16,
    pub base_mint_decimals: u8,
    pub quote_mint_decimals: u8,
    pub pool_bump: u8,
    pub is_mayhem_mode: Bool,
}

/// Field-for-field mirror of the real `pump_amm::InitBoostEvent`. Field
/// order (like `CreatePoolEvent` above) is alignment-optimized, not the
/// real IDL's own listing order — `i128` (16-byte alignment) leads so no
/// padding gets inserted before it.
#[event]
pub struct InitBoostEvent {
    pub virtual_quote_reserves: i128,
    pub timestamp: i64,
    pub real_quote_reserves_after: u64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub pool: Address,
}

/// Field-for-field mirror of the real `pump_amm::BoostBuyAndBurnEvent`.
#[event]
pub struct BoostBuyAndBurnEvent {
    pub virtual_quote_reserves: i128,
    pub timestamp: i64,
    pub quote_amount_in_requested: u64,
    pub quote_amount_in_used: u64,
    pub base_amount_burned: u64,
    pub real_quote_reserves_after: u64,
    pub base_reserves_after: u64,
    pub boost_vault_remaining: u64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub pool: Address,
    pub authority: Address,
}
