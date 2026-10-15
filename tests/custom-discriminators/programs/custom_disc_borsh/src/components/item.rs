use naclac_lang::prelude::*;

// Custom 4-byte account discriminator (the default is 8 bytes,
// sha256-derived).
#[component(discriminator = [9, 9, 9, 9])]
pub struct Item {
    pub owner: Address,
    pub value: u8,
}
