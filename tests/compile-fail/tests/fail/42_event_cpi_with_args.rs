#![allow(unexpected_cfgs)]
// Site: macros/src/event_cpi.rs (via `reject_nonempty_attr`) —
// `#[event_cpi]` takes no arguments.
use naclac_lang::prelude::*;

#[event_cpi(custom_authority)]
#[derive(Accounts)]
pub struct Foo {}

fn main() {}
