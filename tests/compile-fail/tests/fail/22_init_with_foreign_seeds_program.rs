#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/parser.rs — `init`/`init_if_needed`
// combined with `seeds::program = ...` on the same field. The
// account-creation CPI's `invoke_signed` always signs under the currently
// executing program, so a `seeds::program` pointing at a different program
// can never satisfy the signature check — this combination has no valid use.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [b"foo"],
        bump,
        seeds::program = naclac_lang::prelude::SYSTEM_PROGRAM_ID
    )]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
