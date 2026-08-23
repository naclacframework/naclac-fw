use naclac_lang::prelude::*;

#[component]
pub struct Pool {
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub lp_mint: Address,
    pub pool_base_token_account: Address,
    pub pool_quote_token_account: Address,
    pub coin_creator: Address,
    pub lp_supply: u64,
    pub is_mayhem_mode: Bool,
    pub is_cashback_coin: Bool,
    pub virtual_quote_reserves: [u8; 16],
}
