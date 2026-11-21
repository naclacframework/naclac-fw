//! Builds, but assembling the IDL must FAIL: two accounts are both named `Foo`
//! and two `#[error_code]` enums both start at code 6000. The IDL step
//! (`naclac_idl::registry::collect`) reports each; the test runs that step.
#![no_std]
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

pub mod first {
    use naclac_lang::prelude::*;

    #[account]
    pub struct Foo {
        pub value: u64,
    }

    #[error_code]
    pub enum FirstError {
        A,
    }
}

pub mod second {
    use naclac_lang::prelude::*;

    #[account]
    pub struct Foo {
        pub value: u64,
    }

    #[error_code]
    pub enum SecondError {
        B,
    }
}

#[derive(Accounts)]
pub struct Noop {
    pub payer: Signer,
}

#[program]
pub mod idl_name_and_code_clashes {
    pub fn noop(_ctx: Context<Noop>) -> Result {
        Ok(())
    }
}

#[cfg(all(test, feature = "idl-build"))]
extern crate std;

#[cfg(all(test, feature = "idl-build"))]
mod idl_step {
    use std::{panic, string::String};

    #[test]
    fn the_idl_step_reports_the_name_clash_and_the_code_clash() {
        let payload = panic::catch_unwind(|| crate::__naclac_print_idl())
            .expect_err("the IDL step must panic");
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .expect("the panic carries a message");
        assert!(
            message.contains("two IDL account items are both named `Foo`"),
            "{message}"
        );
        assert!(
            message.contains("error code 6000 is used by both"),
            "{message}"
        );
    }
}
