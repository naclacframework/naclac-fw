#![allow(unexpected_cfgs)]
// An Accounts struct needs named fields.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo(Signer);

fn main() {}
