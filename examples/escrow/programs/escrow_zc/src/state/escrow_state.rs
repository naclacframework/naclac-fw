use {crate::constants::SEED_ESCROW, naclac_lang::prelude::*};

#[account]
#[seeds(SEED_ESCROW, maker, id: u64)]
pub struct EscrowState {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub amount_a: u64,
    pub amount_b: u64,
    pub bump: u8,
}
