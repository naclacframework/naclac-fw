#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs:393 — a `seeds = [...]`
// constraint with no `bump` at all. On-chain `find_program_address` is
// strictly banned, so seeds validation always requires a bump to enable the
// hash-and-compare optimization.
//
// Note: this trybuild fixture has no `Naclac.toml`, so `find_program_id`
// (security.rs:812) can never resolve a program ID here — meaning the
// compile-time-literal-PDA precomputation path (which needs both an
// all-literal seed array *and* a resolvable program ID) never activates in
// this crate, regardless of whether the seed itself is a literal. Every
// `seeds = [...]` fixture here exercises the dynamic-PDA branch. A case
// specifically for the *other* branch (`bump = <expr>` combined with `init`
// on a genuinely precomputed literal-seed PDA, security.rs:325-332) needs a
// real program workspace with a working `Naclac.toml`/`declare_id!` instead
// — not testable in isolation here.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"])]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}