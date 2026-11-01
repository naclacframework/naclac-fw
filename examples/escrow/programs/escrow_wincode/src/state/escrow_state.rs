use naclac_lang::prelude::*;

#[account]
pub struct EscrowState {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub amount_a: u64,
    pub amount_b: u64,
    pub bump: u8,
}
