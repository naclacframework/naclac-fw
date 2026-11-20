//! Events of the heap family: every event is encoded with the program's layout.

use {
    crate::types::{Action, Color, Level, Wide},
    naclac_lang::prelude::*,
};

#[event(discriminator = [2, 0, 1])]
pub struct ScalarsSet {
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

#[event(discriminator = [2, 0, 2])]
pub struct EnumsChanged {
    pub color: Color,
    pub action: Action,
    pub level: Level,
    pub wide: Wide,
}

#[event(discriminator = [2, 0, 3])]
pub struct RosterChanged {
    pub scores: Vec<u64>,
    pub name: String,
}

#[event(discriminator = [2, 0, 4])]
pub struct LabLog {
    pub head: u8,
    pub colors: Vec<Color>,
    pub maybe: Option<Color>,
    pub note: String,
    pub values: Vec<u64>,
    pub action: Action,
}
