//! Must FAIL to build: `Foo`'s explicit discriminator is all-zero, which
//! `#[account]` rejects (an all-zero prefix is how an uninitialized account is
//! recognized), in every build and without the `idl-build` feature.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[account(discriminator = [0, 0, 0, 0, 0, 0, 0, 0])]
pub struct Foo {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod zero_account_disc {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}
