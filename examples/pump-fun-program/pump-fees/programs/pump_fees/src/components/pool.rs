use naclac_lang::prelude::*;

// Already deployed on devnet — explicit padding for the two internal gaps
// below, not a field reorder, since this layout is already live. Revisit
// alongside a redeploy (reorder largest-alignment-first, drop these).
#[component]
pub struct Pool {
    pub pool_bump: u8,
    _padding_a: [u8; 1],
    pub index: u16,
    pub creator: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub lp_mint: Address,
    pub pool_base_token_account: Address,
    pub pool_quote_token_account: Address,
    pub coin_creator: Address,
    _padding_b: [u8; 4],
    pub lp_supply: u64,
    pub is_mayhem_mode: Bool,
    pub is_cashback_coin: Bool,
    pub virtual_quote_reserves: [u8; 16],
}
