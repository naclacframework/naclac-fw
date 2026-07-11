//! # Interface Wrappers
//!
//! Provides generic program interface wrappers (Interface, TokenInterface) for
//! dynamic program resolution.

use crate::prelude::AccountInfo;
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::prelude::{Address, NaclacError, Result};
use crate::wrappers::{ToAccountInfo, ToAddress};
use core::ops::Deref;

/// Wrapper for executable program accounts that adhere to an interface with multiple possible IDs.
#[derive(Clone)]
pub struct Interface<T: Clone = ()> {
    #[cfg(not(feature = "pinocchio"))]
    pub info: AccountInfo,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
    pub index: usize,
    pub _phantom: core::marker::PhantomData<T>,
}

/// Unit struct representing the Token Program Interface (accepts standard Token or Token-2022).
#[derive(Clone, Copy)]
pub struct TokenInterface;

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> Interface<T> {
    pub fn try_from(info: &AccountInfo, allowed_ids: &[Address], index: usize) -> Result<Self> {
        if !info.executable {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }

        let mut matched = false;
        for expected_id in allowed_ids {
            if info.key == expected_id {
                matched = true;
                break;
            }
        }

        if !matched {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }

        Ok(Self {
            info: info.clone(),
            index,
            _phantom: core::marker::PhantomData,
        })
    }

    pub fn address(&self) -> Address {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Interface<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Interface<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> crate::wrappers::Owner for Interface<T> {
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> Deref for Interface<T> {
    type Target = AccountInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> ToAccountInfo for Interface<T> {
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> crate::wrappers::ToAccountInfos for Interface<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Clone> ToAddress for Interface<T> {
    fn address(&self) -> Address {
        *self.info.key
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl<T: Clone> Interface<T> {
    pub fn try_from(info: &AccountInfo, allowed_ids: &[Address], index: usize) -> Result<Self> {
        if !info.view.executable() {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }
        let mut matched = false;
        for expected_id in allowed_ids {
            if info.view.address() == expected_id.as_address() {
                matched = true;
                break;
            }
        }

        if !matched {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }
        Ok(Self {
            view: info.view,
            index,
            _phantom: core::marker::PhantomData,
        })
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Interface<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::CpiHandle {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a, T: Clone> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Interface<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::CpiHandleMut {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> crate::wrappers::Owner for Interface<T> {
    fn program_owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> Deref for Interface<T> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> crate::wrappers::ToAccountView for Interface<T> {
    fn to_account_view(&self) -> AccountView {
        self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> ToAccountInfo for Interface<T> {
    fn to_account_info(&self) -> AccountInfo {
        AccountInfo { view: self.view }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: Clone> ToAddress for Interface<T> {
    fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

impl crate::wrappers::Ids for TokenInterface {
    #[inline(always)]
    fn ids() -> &'static [Address] {
        &[
            crate::prelude::TOKEN_PROGRAM_ID,
            crate::prelude::TOKEN_2022_PROGRAM_ID,
        ]
    }
}

impl<T: crate::wrappers::Ids + Clone> crate::wrappers::NaclacAccount for Interface<T> {
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> Result<Self> {
        Self::try_from(info, T::ids(), index)
    }
}
