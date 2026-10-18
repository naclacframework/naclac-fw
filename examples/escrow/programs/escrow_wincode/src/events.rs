use naclac_lang::prelude::*;

#[event]
pub struct EscrowCreated {
    pub maker: Address,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct EscrowExchanged {
    pub taker: Address,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct EscrowCancelled {
    pub maker: Address,
    pub amount_a: u64,
}
