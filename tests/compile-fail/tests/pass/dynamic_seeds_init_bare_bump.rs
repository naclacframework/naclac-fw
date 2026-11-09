#![allow(unexpected_cfgs)]
// Previously a hard `compile_error!` (site: derive/src/accounts/field/checks/
// pda.rs, the `Some(PdaBump::Auto)` arm reached when `init_config.is_some()`)
// — dynamic (non-literal) seeds combined with `init` and bare `bump` had no
// compile-time-precomputed answer available and no client-supplied bump to
// check, so naclac rejected it outright. Now routes through
// `naclac_lang::prelude::find_program_address`'s on-chain canonical search
// instead, the one case naclac's on-chain-search ban doesn't apply to: no
// precomputation is possible for seeds not known until runtime, so this is
// the only way to establish canonicality for a fresh dynamic-seed PDA.
//
// `&[1u8]` is neither a literal nor a constant, so this field takes the
// dynamic-PDA branch.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [b"foo", &[1u8]],
        bump
    )]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub bump: u8,
    pub value: u64,
}

fn main() {}
