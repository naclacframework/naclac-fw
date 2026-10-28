//! `#[component]` account types: a wide field mix, relation targets and seed-backed state.

use crate::constants::ARR_LEN;
use crate::types::{ComponentOnly, Mode, Nested, Pair, Shape, Wrapper};
use naclac_lang::prelude::*;

/// Account with one field of nearly every IDL-visible type.
#[component]
pub struct Vault {
    /// Canonical bump.
    pub bump: u8,
    pub admin: Address,
    pub initiator: Address,
    pub mutator: Address,
    pub balance: u64,
    pub fee_bps: u16,
    pub nonce: u32,
    pub delta: i64,
    pub small: i8,
    pub mid: i16,
    pub wide: i32,
    pub ratio: f32,
    pub price: f64,
    pub active: bool,
    pub huge: u128,
    pub huge_signed: i128,
    pub label: [u8; 8],
    pub keys: [Address; 2],
    pub sized: [u8; ARR_LEN],
    pub grid: [[u8; 2]; 2],
    pub maybe: Option<u64>,
    pub maybe_key: Option<Address>,
    pub maybe_small: Option<u32>,
    pub text: String,
    pub bytes: Vec<u8>,
    pub pairs: Vec<Pair>,
    pub maybe_pair: Option<Pair>,
    pub pair: Pair,
    pub nested: Nested,
    pub wrapper: Wrapper,
    pub deep: ComponentOnly,
    pub mode: Mode,
    pub shape: Shape,
    pub _reserved: [u8; 3],
}

/// Relation targets; two field names contain the substrings `init` and `mut`.
#[component]
pub struct Roles {
    pub bump: u8,
    pub admin: Address,
    pub initiator: Address,
    pub mutator: Address,
    pub authority: Address,
}

#[component]
pub struct Registry {
    pub bump: u8,
    pub label: [u8; 8],
    pub count: u64,
}

#[component]
pub struct Entry {
    pub bump: u8,
    pub value: u64,
}

#[component]
pub struct Ledger {
    pub bump: u8,
    pub total: u64,
}

#[component]
pub struct Growable {
    pub bump: u8,
    pub tag: u8,
}

#[component]
pub struct Tiny {
    pub flag: u8,
}
