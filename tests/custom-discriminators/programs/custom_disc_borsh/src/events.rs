use naclac_lang::prelude::*;

// Custom 1-byte event discriminator (the default is 8 bytes,
// sha256-derived). Events are a separate category from accounts and
// instructions, so this width doesn't need to match either of theirs.
#[event(discriminator = 42)]
pub struct ItemCreated {
    pub owner: Address,
    pub value: u8,
}
