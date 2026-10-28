//! `#[event]` structs: fixed-size (zero-copy wire layout) and `#[event(alloc)]` dynamic ones.

use crate::constants::ARR_LEN;
use crate::types::{EventOnly, Pair};
use naclac_lang::prelude::*;

/// Fixed-size event; fields ordered by descending alignment.
#[event]
pub struct Basic {
    /// Event id.
    pub id: u64,
    pub amount: i64,
    pub who: Address,
    pub flag: bool,
    pub small: u8,
    pub _note: u8,
}

#[event]
pub struct Arrays {
    pub data: [u8; 32],
    pub grid: [[u8; 2]; 2],
    pub sized: [u8; ARR_LEN],
}

#[event]
pub struct WithOpt {
    pub maybe: Option<u64>,
    pub huge: u128,
    pub pair: Pair,
    pub only: EventOnly,
}

#[event]
pub struct Dynamic {
    pub head: u8,
    pub items: Vec<Pair>,
    pub note: String,
    pub maybe: Option<u64>,
    pub bytes: Vec<u8>,
}

#[event]
pub struct DynamicNested {
    pub inner: Pair,
    pub many: Vec<Pair>,
}
