//! Must PASS: the positive-path counterpart to dup_account_disc/ and
//! zero_account_disc/ — `Foo` and `Bar` have distinct, non-zero
//! discriminators, so the crate-wide scan must find no violation and let
//! this build succeed. Without this, a bug that made the scan reject
//! everything (a false positive) would look identical from the outside to
//! it correctly rejecting only real collisions.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[component(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
pub struct Foo {
    pub value: u64,
}

#[component(discriminator = [2, 2, 2, 2, 2, 2, 2, 2])]
pub struct Bar {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod distinct_account_disc {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}
