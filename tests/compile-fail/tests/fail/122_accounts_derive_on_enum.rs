#![allow(unexpected_cfgs)]
// The Accounts derive applies to a struct.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub enum Foo {
    A,
}

fn main() {}
