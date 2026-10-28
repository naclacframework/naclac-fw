//! Loaded through `#[path]` from the crate root; its own child module resolves next to it.

use naclac_lang::prelude::*;

pub mod sub;

#[event]
pub struct MovedEvent {
    pub x: u64,
}

#[constant]
pub const MOVED_CONST: u16 = 9;
