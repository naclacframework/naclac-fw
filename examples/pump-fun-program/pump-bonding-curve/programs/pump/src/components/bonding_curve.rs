use naclac_lang::prelude::*;

#[component]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_quote_reserves: u64,
    pub real_token_reserves: u64,
    pub real_quote_reserves: u64,
    pub token_total_supply: u64,
    pub complete: Bool,
    pub creator: Address,
    pub is_mayhem_mode: Bool,
    pub is_cashback_coin: Bool,
    pub quote_mint: Address,
    pub _reserved_trailing: [u8; 36],
}
