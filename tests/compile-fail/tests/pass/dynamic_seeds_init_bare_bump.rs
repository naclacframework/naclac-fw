#![allow(unexpected_cfgs)]
// Previously a hard `compile_error!` (site: naclac-macros/src/instruction/
// security.rs, the `Some(PdaBump::Auto)` arm reached when `init_config.is_some()`)
// — dynamic (non-literal) seeds combined with `init` and bare `bump` had no
// compile-time-precomputed answer available and no client-supplied bump to
// check, so naclac rejected it outright. Now routes through
// `naclac_lang::prelude::find_program_address`'s on-chain canonical search
// instead, the one case naclac's on-chain-search ban doesn't apply to: no
// precomputation is possible for seeds not known until runtime, so this is
// the only way to establish canonicality for a fresh dynamic-seed PDA.
//
// This fixture crate has no `Naclac.toml`, so the literal-seed
// compile-time-precompute path never activates here (see
// tests/fail/17_seeds_missing_bump.rs's own note) — `[b"foo"]` still
// exercises the dynamic-PDA branch despite looking literal.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [b"foo"],
        bump
    )]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[component]
pub struct FooData {
    pub bump: u8,
    pub value: u64,
}

fn main() {}
