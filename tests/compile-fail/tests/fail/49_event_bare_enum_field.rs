use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

#[event]
pub struct Log {
    pub head: u8,
    pub mode: Mode,
}

fn main() {}
