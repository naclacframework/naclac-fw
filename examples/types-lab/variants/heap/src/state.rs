//! Accounts of the heap family: the lab holds one field of every type.

use {
    crate::types::{Action, Color, Level, Limits, Wide},
    naclac_lang::prelude::*,
};

#[account(discriminator = [3, 0, 0, 1])]
pub struct Lab {
    pub bump: u8,
    pub authority: Address,
    pub flag: bool,
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
    pub color: Color,
    pub action: Action,
    pub level: Level,
    pub wide: Wide,
    pub limits: Limits,
    pub maybe_u64: Option<u64>,
    pub maybe_color: Option<Color>,
    pub history: [Color; 4],
    #[max_len(4)]
    pub scores: Vec<u64>,
    #[max_len(16)]
    pub name: String,
    #[max_len(3)]
    pub members: Vec<Address>,
    #[max_len(8)]
    pub flags: Vec<bool>,
    pub counter: u8,
}

#[account(discriminator = [3, 0, 0, 2])]
pub struct Note {
    pub bump: u8,
    pub value: u64,
}
