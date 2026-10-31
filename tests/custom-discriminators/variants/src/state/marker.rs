use naclac_lang::prelude::*;

// Same discriminator width as `Item` (accounts in one program must share
// one width), but a distinct value — this is what makes `Item` and
// `Marker` distinguishable from each other on-chain.
#[account(discriminator = [7, 7, 7, 7])]
pub struct Marker {
    pub owner: Address,
    pub tag: u8,
}
