use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

#[defined_type]
pub struct Settings {
    pub mode: Mode,
}

fn main() {}
