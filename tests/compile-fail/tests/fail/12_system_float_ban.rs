#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/system.rs:877 (via `FloatBanVisitor`) — `f32`/`f64`
// are banned inside `#[system]` functions: floating-point arithmetic is
// non-deterministic across Solana/SBF validators.
use naclac_lang::prelude::*;

#[system]
pub fn foo(x: f64) -> f64 {
    x
}

fn main() {}