#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/checks/pda.rs (`seeds_type_check`) — a typed
// `seeds = T::seeds(..)` on an `Account<X>` field is accepted only when `T` is
// `X` or a `#[derive(Seeds)]` type, so a field never takes the seeds of
// another account type.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"first")]
pub struct First {
    pub value: u64,
}

#[account]
#[seeds(b"second")]
pub struct Second {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = Second::seeds(), bump)]
    pub account: Account<First>,
}

fn main() {}
