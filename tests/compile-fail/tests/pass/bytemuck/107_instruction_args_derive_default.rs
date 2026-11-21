// Without the derive, the zero-copy layout's own `Default` is used.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args {
    pub amount: u64,
}

fn main() {
    let _ = Args::default();
}
