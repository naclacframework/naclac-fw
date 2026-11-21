use naclac_lang::prelude::*;

#[defined_type]
#[repr(u8)]
pub enum Mode {
    Off,
    On,
}

#[event(alloc)]
pub struct Log {
    pub head: u8,
    pub flag: bool,
    pub note: String,
    pub mode: ZcEnum<Mode>,
    pub modes: Vec<ZcEnum<Mode>>,
    pub maybe: Option<ZcEnum<Mode>>,
}

fn main() {}
