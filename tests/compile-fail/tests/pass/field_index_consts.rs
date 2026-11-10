#![allow(unexpected_cfgs)]
// Valid fixture: every declared field of an `Accounts` struct gets a
// `<FIELD>_INDEX` constant equal to the field's slot in the account list.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub first: Signer,
    pub second: Signer,
    pub third_account: Signer,
}

const _: () = {
    assert!(Foo::FIRST_INDEX == 0);
    assert!(Foo::SECOND_INDEX == 1);
    assert!(Foo::THIRD_ACCOUNT_INDEX == 2);
};

fn main() {}
