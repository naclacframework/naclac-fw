//! Plain enums and a struct holding one, as a Borsh program stores them.

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
pub enum Level {
    Low = 0,
    Mid = 100,
    High = 200,
}

#[defined_type]
pub enum Wide {
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
pub struct ModeArgs {
    pub mode: Color,
    pub amount: u64,
}

#[event]
pub struct StateChanged {
    pub action: Action,
    pub color: Color,
    pub counter: u8,
}
