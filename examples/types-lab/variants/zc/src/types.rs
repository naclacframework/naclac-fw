//! Defined types and instruction argument groups of the zero-copy family.

use naclac_lang::prelude::*;

#[defined_type]
#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Blue = 7,
}

#[defined_type]
#[repr(u8)]
pub enum Action {
    Pause,
    SetFee { fee_bps: u16 },
    Transfer { amount: u64, to: Address },
    Paint(Color),
}

#[defined_type]
#[repr(u16)]
pub enum Level {
    Low = 1,
    Mid = 300,
    High = 65_000,
}

#[defined_type]
#[repr(i8)]
pub enum Signed {
    Neg = -3,
    Zero = 0,
    Pos = 5,
}

#[defined_type]
#[repr(u16)]
pub enum Wide {
    Empty,
    Small(u8),
    Big { value: u64 },
}

#[defined_type]
pub struct Limits {
    pub min: u64,
    pub max: u64,
    pub mode: ZcEnum<Color>,
}

#[defined_type]
pub struct Entry {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
}

#[instruction_args]
pub struct ScalarArgs {
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
