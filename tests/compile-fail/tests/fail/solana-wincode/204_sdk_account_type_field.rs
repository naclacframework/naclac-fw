#![allow(unexpected_cfgs)]
// Site: derive/src/accounts — the Solana SDK's own account type used as an
// Accounts-struct field. Only the framework wrappers implement `NaclacAccount`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub bad: naclac_lang::solana_account_info::AccountInfo<'static>,
}

fn main() {}
