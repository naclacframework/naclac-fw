use naclac_lang::prelude::*;

#[event(discriminator = [9, 8, 7, 6, 5])]
pub struct Ticked {
    pub n: u64,
}
