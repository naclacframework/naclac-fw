// A zero-copy enum must declare its integer `repr` so its tag width is fixed.
use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

fn main() {}
