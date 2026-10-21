use naclac_lang::prelude::*;

#[event(discriminator = [1, 2, 3, 4, 5, 6, 7])]
pub struct Ticked {
    pub n: u64,
}
