use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

#[event(alloc)]
pub struct Log {
    pub head: u8,
    pub mode: Mode,
    pub modes: Vec<Mode>,
}

fn main() {}
