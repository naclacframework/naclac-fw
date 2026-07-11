#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/lib.rs's `check_banned_functions`, invoked from
// the `#[instruction]` attribute — on-chain `find_program_address`/
// `create_program_address` is strictly banned; callers must pass the bump
// from the client and validate via the hash-and-compare `#[account(seeds =
// [...], bump = ...)]` path instead.
use naclac_lang::prelude::*;

#[instruction]
pub fn foo(ctx: Context<FooAccounts>) -> Result {
    let _ = Address::find_program_address(&[b"seed"], &ctx.program_id);
    Ok(())
}

#[derive(Accounts)]
pub struct FooAccounts {}

fn main() {}