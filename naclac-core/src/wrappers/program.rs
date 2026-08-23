//! # Program Wrapper
//!
//! Provides the Program wrapper to enforce that a given account is executable.

use crate::prelude::AccountInfo;
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::prelude::{Address, NaclacError, Result};
use crate::wrappers::{ToAccountInfo, ToAddress};
use core::ops::Deref;

/// Wrapper for executable program accounts.
#[cfg_attr(not(feature = "pinocchio"), derive(Clone))]
pub struct Program<T = ()> {
    #[cfg(not(feature = "pinocchio"))]
    pub info: AccountInfo,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
    pub _phantom: core::marker::PhantomData<T>,
}

#[cfg(feature = "pinocchio")]
impl<T> Clone for Program<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(feature = "pinocchio")]
impl<T> Copy for Program<T> {}

/// Unit struct representing the Solana System Program.
#[derive(Clone, Copy)]
pub struct System;

/// Unit struct representing the SPL Token Program.
#[derive(Clone, Copy)]
pub struct Token;

/// Unit struct representing the SPL Token-2022 Program.
#[derive(Clone, Copy)]
pub struct Token2022;

/// Unit struct representing the SPL Associated Token Program.
#[derive(Clone, Copy)]
pub struct AssociatedToken;

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> Program<T> {
    pub fn try_from(
        info: &AccountInfo,
        expected_program_id: &Address,
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
            _phantom: core::marker::PhantomData,
        })
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> crate::wrappers::Owner for Program<T> {
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> Deref for Program<T> {
    type Target = AccountInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> ToAccountInfo for Program<T> {
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> crate::wrappers::ToAccountInfos for Program<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> ToAddress for Program<T> {
    fn address(&self) -> Address {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> Program<T> {
    pub fn address(&self) -> Address {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Program<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Program<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl<T: Clone> Program<T> {
    pub fn try_from(
        info: &AccountInfo,
        expected_program_id: &Address,
        index: usize,
    ) -> Result<Self> {
        if !info.view.executable() {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }
        if info.view.address() != expected_program_id.as_address() {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }
        Ok(Self {
            view: info.view,
            _phantom: core::marker::PhantomData,
        })
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Program<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::CpiHandle {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Program<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::CpiHandleMut {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> crate::wrappers::Owner for Program<T> {
    fn program_owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> Deref for Program<T> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> crate::wrappers::ToAccountView for Program<T> {
    fn to_account_view(&self) -> AccountView {
        self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> ToAccountInfo for Program<T> {
    fn to_account_info(&self) -> AccountInfo {
        AccountInfo { view: self.view }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> ToAddress for Program<T> {
    fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

impl crate::wrappers::Id for System {
    #[inline(always)]
    fn id() -> Address {
        crate::prelude::SYSTEM_PROGRAM_ID
    }
}

impl crate::wrappers::Id for Token {
    #[inline(always)]
    fn id() -> Address {
        crate::prelude::TOKEN_PROGRAM_ID
    }
}

impl crate::wrappers::Id for Token2022 {
    #[inline(always)]
    fn id() -> Address {
        crate::prelude::TOKEN_2022_PROGRAM_ID
    }
}

impl crate::wrappers::Id for AssociatedToken {
    #[inline(always)]
    fn id() -> Address {
        crate::prelude::ASSOCIATED_TOKEN_PROGRAM_ID
    }
}

impl<T: crate::wrappers::Id + Clone> crate::wrappers::NaclacAccount for Program<T> {
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> Result<Self> {
        Self::try_from(info, &T::id(), index)
    }
}
