//! The account whose fixed-capacity fields the tests exercise.

use naclac_lang::prelude::*;

#[account]
pub struct Profile {
    pub bump: u8,
    pub owner: Address,
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<16>,
    pub members: FixedVec<Address, 3>,
    pub flags: FixedVec<Bool, 8>,
    pub counter: u16,
}
