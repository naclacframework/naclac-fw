//! A `pub(crate)` module nested below the crate root: reachable from the IDL assembler.

use naclac_lang::prelude::*;

#[event]
pub struct CrateVisibleEvent {
    pub flag: u8,
}
