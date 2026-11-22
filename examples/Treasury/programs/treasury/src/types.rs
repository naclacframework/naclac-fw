//! Defined types stored in the treasury's accounts or passed as instruction
//! arguments.

use naclac_lang::prelude::*;

#[defined_type]
#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum Status {
    Active,
    Paused,
}
