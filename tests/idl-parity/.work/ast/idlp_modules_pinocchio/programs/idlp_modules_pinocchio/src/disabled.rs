//! Compiled out (`feature = "debug-mode"` is not enabled): must not appear in either IDL.

use naclac_lang::prelude::*;

#[event]
pub struct DisabledEvent {
    pub off: u8,
}
