#![allow(unexpected_cfgs)]
// Site: macros/src/event_cpi.rs — `#[event_cpi]` injects two *named*
// fields (`event_authority`, `program`) into the struct, so it needs a struct
// with named fields.
use naclac_lang::prelude::*;

#[event_cpi]
#[derive(Accounts)]
pub struct Foo(u8);

fn main() {}
