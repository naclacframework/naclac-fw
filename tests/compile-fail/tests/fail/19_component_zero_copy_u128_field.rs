#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/component.rs's `const _: () = assert!(align_of::<Self>() <= 8, ...)`
// — a `u128` field (16-byte aligned) inside a zero-copy `#[component]`.
// `Account<T>`'s zero-copy branch casts the account's raw data pointer straight to `&T`,
// relying only on Solana's 8-byte account-buffer alignment guarantee; a
// field requiring more than 8 bytes of alignment makes that cast undefined
// behavior.
use naclac_lang::prelude::*;

#[component]
pub struct Foo {
    pub bad: u128,
}

fn main() {}
