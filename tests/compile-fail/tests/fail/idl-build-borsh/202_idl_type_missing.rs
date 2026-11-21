#![allow(unexpected_cfgs)]
// Under `idl-build` every account field has an IDL type.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct FooData {
    pub value: core::num::NonZeroU8,
}

fn main() {}
