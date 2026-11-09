//! Builds, but assembling the IDL must FAIL: `Foo` and `Bar` are two different
//! `#[account]` accounts with the identical explicit discriminator, which
//! neither struct's own macro expansion can see. The IDL step
//! (`naclac_idl::registry::collect`) reports it, and the test below runs that
//! step: `cargo test --features idl-build` passes only if it panics with the
//! clash.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[account(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
pub struct Foo {
    pub value: u64,
}

#[account(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
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

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    #[test]
    #[should_panic(expected = "have the same discriminator")]
    fn two_accounts_with_one_discriminator_fail_the_idl_step() {
        crate::__naclac_print_idl();
    }
}
