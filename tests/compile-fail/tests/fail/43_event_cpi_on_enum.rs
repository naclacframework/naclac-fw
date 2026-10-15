#![allow(unexpected_cfgs)]
// Site: macros/src/event_cpi.rs — `#[event_cpi]` only applies to an
// accounts struct; anything else is rejected by parsing the item as a struct.
use naclac_lang::prelude::*;

#[event_cpi]
pub enum Foo {
    A,
}

fn main() {}
