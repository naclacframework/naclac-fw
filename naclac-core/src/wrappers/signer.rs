//! # Signer Wrapper
//!
//! Provides the Signer wrapper to enforce that a given account has signed the transaction.

use crate::prelude::AccountInfo;
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::prelude::{Address, NaclacError, Result};
use crate::wrappers::{ToAccountInfo, ToAddress};
use core::ops::Deref;

/// Wrapper for accounts that must be signers.
#[derive(Clone)]
#[cfg_attr(feature = "pinocchio", derive(Copy))]
pub struct Signer {
    #[cfg(not(feature = "pinocchio"))]
    pub info: AccountInfo,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
}

#[cfg(not(feature = "pinocchio"))]
impl Signer {
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        if !info.is_signer {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { info: info.clone() })
    }
}

#[cfg(not(feature = "pinocchio"))]
impl crate::wrappers::Owner for Signer {
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl Deref for Signer {
    type Target = AccountInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToAccountInfo for Signer {
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl crate::wrappers::ToAccountInfos for Signer {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToAddress for Signer {
    fn address(&self) -> Address {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl Signer {
    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        self.info.sub_lamports(amount)
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        self.info.add_lamports(amount)
    }

    pub fn address(&self) -> Address {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Signer {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Signer {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl crate::wrappers::AsRefByteSlice for Signer {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl Signer {
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        if !info.is_signer() {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { view: info.view })
    }
}

#[cfg(feature = "pinocchio")]
impl Deref for Signer {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl crate::wrappers::ToAccountView for Signer {
    fn to_account_view(&self) -> AccountView {
        self.view
    }
}

#[cfg(feature = "pinocchio")]
impl ToAccountInfo for Signer {
    fn to_account_info(&self) -> AccountInfo {
        AccountInfo { view: self.view }
    }
}

#[cfg(feature = "pinocchio")]
impl crate::wrappers::Owner for Signer {
    fn program_owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }
}

#[cfg(feature = "pinocchio")]
impl ToAddress for Signer {
    fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl Signer {
    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_sub(amount)
            .ok_or(NaclacError::InsufficientFunds.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_add(amount)
            .ok_or(NaclacError::ArithmeticOverflow.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Signer {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::CpiHandle {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Signer {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::CpiHandleMut {
            info: AccountInfo { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl crate::wrappers::AsRefByteSlice for Signer {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.view.address().as_ref()
    }
}

impl crate::wrappers::NaclacAccount for Signer {
    const IS_SIGNER: bool = true;

    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }
}
