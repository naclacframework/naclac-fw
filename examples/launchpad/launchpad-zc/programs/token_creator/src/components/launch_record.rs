use naclac_lang::prelude::*;

#[component]
pub struct LaunchRecord {
    pub creator: Address,
    pub mint: Address,
    pub amount_token: u64,
    pub amount_quote: u64,
}
