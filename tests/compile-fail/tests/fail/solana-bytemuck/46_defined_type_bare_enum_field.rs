use naclac_lang::prelude::*;

#[defined_type] #[repr(u8)]
pub enum Mode {
    Off,
    On,
}

#[defined_type]
pub struct Settings {
    pub mode: Mode,
}

fn main() {}
