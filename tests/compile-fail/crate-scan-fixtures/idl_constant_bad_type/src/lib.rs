//! Builds, but assembling the IDL must FAIL: a `#[constant]` of a defined type
//! has no IDL text. The test runs the IDL step.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[defined_type]
pub struct Spec {
    pub a: u8,
}

#[constant]
pub const SPEC: Spec = Spec { a: 1 };

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod idl_constant_bad_type {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    #[test]
    #[should_panic(expected = "cannot be the type of an IDL constant")]
    fn a_defined_type_constant_fails_the_idl_step() {
        crate::__naclac_print_idl();
    }
}
