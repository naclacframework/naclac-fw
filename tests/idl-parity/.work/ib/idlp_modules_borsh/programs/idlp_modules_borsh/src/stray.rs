//! Never declared with `mod`, so not part of the crate: must not appear in either IDL.

use naclac_lang::prelude::*;

#[event]
pub struct StrayEvent {
    pub lost: u8,
}
