use naclac_lang::prelude::*;

#[defined_type] #[repr(u8)]
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
