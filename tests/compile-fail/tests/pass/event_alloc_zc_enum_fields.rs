use naclac_lang::prelude::*;

#[defined_type]
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
