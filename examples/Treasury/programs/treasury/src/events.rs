//! Events the treasury emits: two through the program log and one through a
//! self-CPI.

use naclac_lang::prelude::*;

#[event(discriminator = 1)]
pub struct TreasuryCreated {
    pub admin: Address,
    pub threshold: u8,
}

#[event(discriminator = 2)]
pub struct Deposited {
    pub depositor: Address,
    pub amount: u64,
}

#[event(discriminator = 3)]
pub struct ProposalExecuted {
    pub id: u64,
    pub amount: u64,
    pub recipient: Address,
}
