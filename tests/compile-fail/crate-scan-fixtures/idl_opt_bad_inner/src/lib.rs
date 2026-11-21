//! Builds, but assembling the IDL must FAIL: `Opt<FixedString<8>>` has no IDL
//! representation, since an `Opt<T>` holds a primitive, `Address`, an array or a
//! defined type. The test runs the IDL step.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[account]
pub struct Foo {
    pub maybe: Opt<FixedString<8>>,
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod idl_opt_bad_inner {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    #[test]
    #[should_panic(expected = "has no IDL representation for this `T`")]
    fn an_opt_of_a_string_fails_the_idl_step() {
        crate::__naclac_print_idl();
    }
}
