//! Enums and a struct holding one, in every shape a zero-copy program stores them.

use naclac_lang::prelude::*;

#[defined_type]
#[derive(PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Blue = 7,
}

#[defined_type]
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

#[instruction_args]
pub struct ModeArgs {
    pub mode: ZcEnum<Color>,
    pub amount: u64,
}

#[instruction_args]
pub struct DynArgs {
    pub tag: u8,
    pub values: ZcVec<u32>,
    pub note: ZcString,
    pub maybe: Option<u16>,
    #[allow_heap]
    pub heap: Vec<u16>,
}

#[instruction_args]
pub struct ListArgs {
    #[allow_heap]
    pub words: Vec<String>,
}

#[event]
pub struct StateChanged {
    pub action: ZcEnum<Action>,
    pub color: ZcEnum<Color>,
    pub counter: u8,
}

#[event]
pub struct MixedEvent {
    pub tag: u8,
    pub amount: u64,
    pub color: ZcEnum<Color>,
}

#[event(alloc)]
pub struct StateLog {
    pub head: u8,
    pub color: ZcEnum<Color>,
    pub colors: Vec<ZcEnum<Color>>,
    pub maybe: Option<ZcEnum<Color>>,
    pub action: ZcEnum<Action>,
}
