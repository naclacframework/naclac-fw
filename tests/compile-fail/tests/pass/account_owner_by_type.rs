#![allow(unexpected_cfgs)]
// Whether `#[derive(Accounts)]` checks a field's owner and signer flag comes
// from its type's `NaclacAccount` constants, so it holds for an alias too.
use naclac_lang::prelude::*;

#[component]
pub struct Foo {
    pub value: u64,
}

type Pool = Account<Foo>;

const _: () = assert!(<Account<Foo> as NaclacAccount>::PROGRAM_OWNED);
const _: () = assert!(<Pool as NaclacAccount>::PROGRAM_OWNED);
const _: () = assert!(!<Signer as NaclacAccount>::PROGRAM_OWNED);
const _: () = assert!(<Signer as NaclacAccount>::IS_SIGNER);
const _: () = assert!(!<Pool as NaclacAccount>::IS_SIGNER);
const _: () = assert!(<Pool as NaclacAccount>::OWNER_VALIDATED);
const _: () = assert!(!<Signer as NaclacAccount>::OWNER_VALIDATED);
const _: () = assert!(<Program<System> as NaclacAccount>::CHECKS_PROGRAM_ID);
const _: () = assert!(!<Pool as NaclacAccount>::CHECKS_PROGRAM_ID);

#[derive(Accounts)]
pub struct Touch {
    pub authority: Signer,
    pub pool: Pool,
}

fn main() {}
