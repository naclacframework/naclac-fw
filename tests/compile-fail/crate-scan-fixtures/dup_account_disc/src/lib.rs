//! Must FAIL to build: `Foo` and `Bar` are two different `#[component]`
//! accounts with the identical explicit discriminator. This is only
//! catchable by `#[program]`'s crate-wide scan (naclac_syn::discriminator::
//! scan_crate_discriminators + validate_category) — it can't be caught by
//! either struct's own macro expansion in isolation, since neither knows
//! the other exists. A real `src/` tree (unlike a single-file trybuild
//! fixture) is required for that scan to see both structs at once — see
//! docs/plan/custom-discriminators.md for why this can't live in
//! tests/compile-fail/tests/fail/ alongside the other fixtures.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[component(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
pub struct Foo {
    pub value: u64,
}

#[component(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
pub struct Bar {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod dup_account_disc {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}
