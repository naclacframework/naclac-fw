//! Accounts of the zero-copy family: the lab holds one field of every type.

use {
    crate::types::{Action, Color, Entry, Level, Limits, Signed, Wide},
    naclac_lang::prelude::*,
};

#[account(discriminator = [3, 0, 0, 1])]
pub struct Lab {
    pub bump: u8,
    pub authority: Address,
    pub flag: Bool,
    pub small: u8,
    pub short: u16,
    pub medium: u32,
    pub large: u64,
    pub huge: u128,
    pub tiny_signed: i8,
    pub short_signed: i16,
    pub medium_signed: i32,
    pub large_signed: i64,
    pub huge_signed: i128,
    pub single: f32,
    pub double: f64,
    pub key: Address,
    pub bytes: [u8; 8],
    pub matrix: [u16; 4],
    pub color: ZcEnum<Color>,
    pub action: ZcEnum<Action>,
    pub level: ZcEnum<Level>,
    pub signed: ZcEnum<Signed>,
    pub wide: ZcEnum<Wide>,
    pub limits: Limits,
    pub maybe_u64: Opt<u64>,
    pub maybe_color: Opt<ZcEnum<Color>>,
    pub history: [ZcEnum<Color>; 4],
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<16>,
    pub members: FixedVec<Address, 3>,
    pub flags: FixedVec<Bool, 8>,
    pub counter: u8,
}

#[account(discriminator = [3, 0, 0, 2])]
pub struct Note {
    pub bump: u8,
    pub value: u64,
}

#[account(discriminator = [3, 0, 0, 3])]
pub struct Book {
    pub bump: u8,
    pub first: Entry,
    pub all: [Entry; 2],
}
