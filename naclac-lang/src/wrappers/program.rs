//! # Program Wrapper
//!
//! Provides the Program wrapper to enforce that a given account is executable.

use crate::prelude::AccountInfo;
use crate::prelude::{Pubkey, NaclacError, Result};
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::wrappers::{ToAccountInfo, ToPubkey};
use core::ops::Deref;

/// Wrapper for executable program accounts.
#[derive(Clone)]
pub struct Program<'info, T: Clone = ()> {
    #[cfg(not(feature = "pinocchio"))]
    pub info: AccountInfo<'info>,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
    pub index: usize,
    #[cfg(not(feature = "pinocchio"))]
    pub _phantom: core::marker::PhantomData<T>,
    #[cfg(feature = "pinocchio")]
    pub _phantom: core::marker::PhantomData<(&'info (), T)>,
}

/// Unit struct representing the Solana System Program.
#[derive(Clone, Copy)]
pub struct System;

/// Unit struct representing the SPL Token Program.
#[derive(Clone, Copy)]
pub struct Token;

/// Unit struct representing the SPL Token-2022 Program.
#[derive(Clone, Copy)]
pub struct Token2022;

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> Program<'info, T> {
    pub fn try_from(
        info: &AccountInfo<'info>,
        expected_program_id: &Pubkey,
        index: usize,
    ) -> Result<Self> {
        if !info.executable {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }
        if info.key != expected_program_id {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }
        Ok(Self {
            info: info.clone(),
            index,
            _phantom: core::marker::PhantomData,
        })
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::Key for Program<'info, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::Owner for Program<'info, T> {
    fn owner(&self) -> Pubkey {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> Deref for Program<'info, T> {
    type Target = AccountInfo<'info>;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> ToAccountInfo<'info> for Program<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::ToAccountInfos<'info> for Program<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> ToPubkey for Program<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        *self.info.key
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> Program<'info, T> {
    pub fn try_from(
        info: &AccountView,
        expected_program_id: &Pubkey,
        index: usize,
    ) -> Result<Self> {
        if !info.executable() {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }
        if info.address() != expected_program_id.as_address() {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }
        Ok(Self { view: info.clone(), index, _phantom: core::marker::PhantomData })
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::Key for Program<'info, T> {
    fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::Owner for Program<'info, T> {
    fn owner(&self) -> Pubkey {
        // SAFETY: Direct sysvar/SBF call via Pinocchio.
        // SAFETY: We are guaranteed the AccountView is initialized directly by the runtime.
        Pubkey::from_address(unsafe { self.view.owner() })
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> Deref for Program<'info, T> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::ToAccountView for Program<'info, T> {
    fn to_account_view(&self) -> AccountView {
        self.view.clone()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> ToAccountInfo<'info> for Program<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        AccountInfo {
            view: self.view.clone(),
            _phantom: core::marker::PhantomData,
        }
    }
}


#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> ToPubkey for Program<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}
