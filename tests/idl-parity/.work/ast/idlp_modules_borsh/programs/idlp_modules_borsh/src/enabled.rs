//! Compiled in (`not(feature = "debug-mode")`), with one item of its own gated off and one on.

use naclac_lang::prelude::*;

#[event]
pub struct EnabledEvent {
    pub on: u8,
}

#[cfg(feature = "debug-mode")]
#[event]
pub struct ItemGatedOff {
    pub off: u8,
}

#[cfg(not(feature = "debug-mode"))]
#[constant]
pub const ITEM_GATED_ON: u8 = 1;
