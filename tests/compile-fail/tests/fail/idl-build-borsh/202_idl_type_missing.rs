#![allow(unexpected_cfgs)]
// Under `idl-build` every account field has an IDL type, which a hand-written type lacks.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Clone, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "naclac_lang::prelude::borsh")]
pub struct Level(u8);

impl SerializedBounds for Level {
    const MAX_SIZE: usize = 1;

    fn within_bounds(&self) -> bool {
        true
    }
}

#[account]
pub struct FooData {
    pub level: Level,
}

fn main() {}
