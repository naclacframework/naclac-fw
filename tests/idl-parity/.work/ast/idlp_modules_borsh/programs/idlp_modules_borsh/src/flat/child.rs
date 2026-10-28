//! Child of a flat file module.

use naclac_lang::prelude::*;

#[defined_type]
pub enum ChildMode {
    A,
    B = 3,
}

#[event]
pub struct ChildEvent {
    pub value: u16,
}
