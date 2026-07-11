#![cfg_attr(feature = "pinocchio", no_std)]

pub(crate) use naclac_core::wrappers;

#[cfg(feature = "solana")]
pub(crate) use naclac_core::cpi;

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub(crate) use naclac_core::borsh;

#[cfg(feature = "pinocchio")]
extern crate alloc;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod token;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod associated_token;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub use token::*;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub use associated_token::Create as CreateAta;

pub mod prelude {
    pub use naclac_core::prelude::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::token::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::associated_token::Create as CreateAta;

    #[cfg(feature = "pinocchio")]
    pub use crate::{pinocchio_associated_token_account, pinocchio_token, pinocchio_token_2022};

    #[cfg(all(feature = "solana", not(feature = "pinocchio")))]
    pub use crate::{spl_associated_token_account, spl_token, spl_token_2022};
}

#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_associated_token_account;
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_token;
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_token_2022;

#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_associated_token_account;
#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_token;
#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_token_2022;
