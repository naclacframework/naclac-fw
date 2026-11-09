use naclac_lang::prelude::*;

#[defined_type] #[repr(u8)]
pub enum Mode {
    Off,
    On,
}

#[instruction_args]
pub struct Args {
    pub mode: Mode,
    pub amount: u64,
}

fn main() {}
