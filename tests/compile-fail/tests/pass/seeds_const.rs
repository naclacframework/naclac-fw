#![allow(unexpected_cfgs)]
// Seeds that are literals or ALL_CAPS constants make the compiler evaluate the
// PDA in a `const`, for every constant type a seed is written with, and for a
// constant `seeds::program`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

pub const SEED_SLICE: &[u8] = b"slice";
pub const SEED_ARRAY: [u8; 5] = *b"array";
pub const SEED_REFERENCE: &[u8; 3] = b"ref";
pub const SEED_STR: &str = "str";
pub const OTHER_PROGRAM: Address = naclac_lang::address!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo", "text", SEED_SLICE, SEED_ARRAY, SEED_REFERENCE], bump)]
    pub account: Account<FooData>,

    #[account(seeds = [SEED_STR.as_bytes(), SEED_SLICE.as_ref()], bump)]
    pub second: Account<FooData>,

    #[account(seeds = [SEED_SLICE], bump, seeds::program = OTHER_PROGRAM)]
    pub foreign: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
