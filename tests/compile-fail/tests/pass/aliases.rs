#![allow(unexpected_cfgs)]
// Type aliases behave like the types they name, without the macros reading
// the crate's source: a documented alias of `AccountInfo` is accepted, an
// alias of `Account<T>` keeps its checks, and an alias of an integer is
// accepted by #[system].
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[account]
pub struct Foo {
    pub value: u64,
}

type Raw = AccountInfo;
type Pool = Account<Foo>;
type Amount = u64;

#[derive(Accounts)]
pub struct Touch {
    /// SAFETY: test fixture; only the address is read.
    pub raw: Raw,
    #[account(mut)]
    pub pool: Pool,
    #[account(seeds = [b"pool", &[1u8]])]
    pub seeded: Pool,
}

#[system]
pub fn double(amount: Amount) -> Result<Amount> {
    Ok(amount * 2)
}

fn main() {}
