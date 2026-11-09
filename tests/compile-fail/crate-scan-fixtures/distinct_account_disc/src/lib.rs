//! Must PASS: the positive-path counterpart to dup_account_disc/ and
//! zero_account_disc/. `Foo` and `Bar` have distinct, non-zero
//! discriminators, so the IDL step must report no clash and
//! `cargo test --features idl-build` must pass. Without this, a bug that made
//! the IDL step reject everything would look identical from the outside to it
//! correctly rejecting only real collisions.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[account(discriminator = [1, 1, 1, 1, 1, 1, 1, 1])]
pub struct Foo {
    pub value: u64,
}

#[account(discriminator = [2, 2, 2, 2, 2, 2, 2, 2])]
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

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    #[test]
    fn distinct_discriminators_assemble_an_idl() {
        crate::__naclac_print_idl();
    }
}
