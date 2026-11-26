//! # Signer Wrapper
//!
//! Provides the Signer wrapper to enforce that a given account has signed the transaction.

#[cfg(feature = "pinocchio")]
use pinocchio::AccountView;
use {
    crate::{
        prelude::{Address, NaclacError, Result, UncheckedAccount},
        wrappers::{ToAddress, ToUncheckedAccount},
    },
    core::ops::Deref,
};

/// Wrapper for accounts that must be signers.
#[derive(Clone)]
#[cfg_attr(feature = "pinocchio", derive(Copy))]
pub struct Signer {
    #[cfg(not(feature = "pinocchio"))]
    pub info: UncheckedAccount,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
}

#[cfg(not(feature = "pinocchio"))]
impl Signer {
    pub fn try_from(info: &UncheckedAccount, index: usize) -> Result<Self> {
        if !info.is_signer {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { info: *info })
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
    type Target = UncheckedAccount;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToUncheckedAccount for Signer {
    fn to_unchecked_account(&self) -> UncheckedAccount {
        self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl crate::wrappers::ToUncheckedAccounts for Signer {
    fn to_unchecked_accounts(&self) -> crate::prelude::Vec<UncheckedAccount> {
        crate::prelude::vec![self.info]
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
    pub fn try_from(info: &UncheckedAccount, index: usize) -> Result<Self> {
        if !info.is_signer() {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { view: info.view })
    }

    /// Wraps `info` without re-checking `is_signer`. Used by generated
    /// account loaders for a slot whose signer flag the declared-account walk
    /// has already enforced.
    ///
    /// # Safety
    ///
    /// `info` must be a signer of the current instruction.
    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn from_verified_signer(info: &UncheckedAccount) -> Self {
        Self { view: info.view }
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
impl crate::wrappers::ToRuntimeView for Signer {
    fn to_runtime_view(&self) -> AccountView {
        self.view
    }
}

#[cfg(feature = "pinocchio")]
impl ToUncheckedAccount for Signer {
    fn to_unchecked_account(&self) -> UncheckedAccount {
        UncheckedAccount { view: self.view }
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
            info: UncheckedAccount { view: self.view },
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Signer {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::CpiHandleMut {
            info: UncheckedAccount { view: self.view },
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
    fn try_from(info: &UncheckedAccount, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }
}
