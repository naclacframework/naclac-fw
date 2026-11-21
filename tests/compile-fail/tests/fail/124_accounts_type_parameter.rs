#![allow(unexpected_cfgs)]
// An Accounts struct takes lifetime parameters only.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo<T> {
    pub a: Signer,
    pub marker: core::marker::PhantomData<T>,
}

fn main() {}
