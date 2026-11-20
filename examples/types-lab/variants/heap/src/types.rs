//! Defined types and instruction argument groups of the heap family (borsh and wincode).

use naclac_lang::prelude::*;

#[defined_type]
#[derive(Copy, Default)]
pub enum Color {
    #[default]
    Red,
    Green,
    Blue = 7,
}

#[defined_type]
#[derive(Copy, Default)]
pub enum Action {
    #[default]
    Pause,
    SetFee { fee_bps: u16 },
    Transfer { amount: u64, to: Address },
    Paint(Color),
}

#[defined_type]
#[derive(Copy, Default)]
pub enum Level {
    #[default]
    Low = 0,
    Mid = 100,
    High = 200,
}

#[defined_type]
#[derive(Copy, Default)]
pub enum Wide {
    #[default]
    Empty,
    Small(u8),
    Big { value: u64 },
}

#[defined_type]
pub struct Limits {
    pub min: u64,
    pub max: u64,
    pub mode: Color,
}

#[instruction_args]
pub struct ScalarArgs {
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
}

#[instruction_args]
pub struct NestedArgs {
    pub limits: Limits,
    pub matrix: [u16; 4],
    pub bytes: [u8; 8],
}

#[instruction_args]
pub struct OptionArgs {
    pub maybe: Option<u64>,
}
