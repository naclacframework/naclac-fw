use naclac_lang::prelude::*;

// Custom 4-byte account discriminator (the default is 8 bytes,
// sha256-derived). `owner`/`value` are both alignment-1 fields, so this
// width is valid for the zero-copy backend too — the discriminator width
// must be a multiple of the account struct's own alignment.
#[account(discriminator = [9, 9, 9, 9])]
pub struct Item {
    pub owner: Address,
    pub value: u8,
}
