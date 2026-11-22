//! A per-member counter that proposals may bump and that can be resized.

use {crate::constants::SEED_NOTE, naclac_lang::prelude::*};

#[account(discriminator = 3)]
#[seeds(SEED_NOTE, authority, id, bump = bump)]
pub struct Note {
    pub authority: Address,
    pub id: u64,
    pub count: u64,
    pub bump: u8,
}
