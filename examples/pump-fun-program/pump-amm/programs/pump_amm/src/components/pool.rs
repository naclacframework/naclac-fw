use naclac_lang::prelude::*;

/// Field-for-field mirror of the real `pump_amm::Pool` account (13 fields,
/// 261 bytes with discriminator) — confirmed against a freshly re-dumped
/// `pump_amm.so` (see `docs/plan/fees-07-donation-relay-progress.md`).
/// `pump_fees`'s own `Pool` mirror depends on this exact layout matching;
/// any change here must be mirrored there too.
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
