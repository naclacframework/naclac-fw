//! Events of the zero-copy family: fixed events and `#[event(alloc)]` events.

use {
    crate::types::{Action, Color, Level, Signed, Wide},
    naclac_lang::prelude::*,
};

#[event(discriminator = [2, 0, 1])]
pub struct ScalarsSet {
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

#[event(discriminator = [2, 0, 2])]
pub struct EnumsChanged {
    pub color: ZcEnum<Color>,
    pub action: ZcEnum<Action>,
    pub level: ZcEnum<Level>,
    pub signed: ZcEnum<Signed>,
    pub wide: ZcEnum<Wide>,
}

#[event(discriminator = [2, 0, 3])]
pub struct RosterChanged {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<16>,
}

#[event(alloc, discriminator = [2, 0, 4])]
pub struct LabLog {
    pub head: u8,
    pub colors: Vec<ZcEnum<Color>>,
    pub maybe: Option<ZcEnum<Color>>,
    pub note: String,
    pub values: Vec<u64>,
    pub action: ZcEnum<Action>,
}

#[event(alloc, discriminator = [2, 0, 5])]
pub struct TextLog {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<16>,
    pub note: String,
}
