//! Token creator event log definitions.

use naclac_lang::prelude::*;

#[event]
pub struct TokenLaunched {
    pub id: u64,
    pub mint: Address,
    pub amount_token: u64,
    pub amount_quote: u64,
}
