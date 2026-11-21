//! Builds, but assembling the IDL must FAIL: the IDL names an `Opt<u64>`
//! `OptU64`, and a user type of that name has a different shape. The test runs
//! the IDL step.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[defined_type]
pub struct OptU64 {
    pub x: u8,
}

#[account]
pub struct Foo {
    pub maybe: Opt<u64>,
    pub other: OptU64,
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod idl_opt_name_clash {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    #[test]
    #[should_panic(expected = "has the name naclac's IDL gives an `Opt<T>`")]
    fn a_user_type_named_like_an_opt_fails_the_idl_step() {
        crate::__naclac_print_idl();
    }
}
