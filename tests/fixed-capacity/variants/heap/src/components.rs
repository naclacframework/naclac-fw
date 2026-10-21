//! The account whose `#[max_len]` sizing the tests check.

use naclac_lang::prelude::*;

#[component]
pub struct Roster {
    pub bump: u8,
    #[max_len(5)]
    pub members: Vec<Address>,
    #[max_len(12)]
    pub label: String,
    pub counter: u32,
}
