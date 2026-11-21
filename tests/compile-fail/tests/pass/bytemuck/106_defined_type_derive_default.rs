// Without the derive, the zero-copy layout's own `Default` is used.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub a: u8,
}

fn main() {
    let _ = Settings::default();
}
