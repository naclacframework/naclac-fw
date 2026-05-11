//! # Signer Wrapper
//!
//! Provides the Signer wrapper to enforce that a given account has signed the transaction.

use crate::prelude::AccountInfo;
use crate::prelude::{Pubkey, NaclacError, Result};
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::wrappers::{ToAccountInfo, ToPubkey};
use core::ops::Deref;

/// Wrapper for accounts that must be signers.
#[derive(Clone)]
pub struct Signer<'info> {
    #[cfg(not(feature = "pinocchio"))]
    pub info: AccountInfo<'info>,
    #[cfg(feature = "pinocchio")]
    pub view: AccountView,
    pub index: usize,
    #[cfg(feature = "pinocchio")]
    pub _phantom: core::marker::PhantomData<&'info ()>,
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> Signer<'info> {
    pub fn try_from(info: &AccountInfo<'info>, index: usize) -> Result<Self> {
        if !info.is_signer {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { info: info.clone(), index })
    }

    pub fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> crate::wrappers::Key for Signer<'info> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> crate::wrappers::Owner for Signer<'info> {
    fn owner(&self) -> Pubkey {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> Deref for Signer<'info> {
    type Target = AccountInfo<'info>;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> ToAccountInfo<'info> for Signer<'info> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> crate::wrappers::ToAccountInfos<'info> for Signer<'info> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> ToPubkey for Signer<'info> {
    fn to_pubkey(&self) -> Pubkey {
        *self.info.key
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl<'info> Signer<'info> {
    pub fn try_from(info: &AccountView, index: usize) -> Result<Self> {
        if !info.is_signer() {
            return Err(NaclacError::ConstraintSigner.err(index));
        }
        Ok(Self { view: info.clone(), index, _phantom: core::marker::PhantomData })
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> Deref for Signer<'info> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> crate::wrappers::ToAccountView for Signer<'info> {
    fn to_account_view(&self) -> AccountView {
        self.view.clone()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToAccountInfo<'info> for Signer<'info> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        AccountInfo {
            view: self.view.clone(),
            _phantom: core::marker::PhantomData,
        }
    }
}


#[cfg(feature = "pinocchio")]
impl<'info> crate::wrappers::Key for Signer<'info> {
    fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> crate::wrappers::Owner for Signer<'info> {
    fn owner(&self) -> Pubkey {
        // SAFETY: Direct sysvar/SBF call via Pinocchio.
        // SAFETY: We are guaranteed the AccountView is initialized directly by the runtime.
        Pubkey::from_address(unsafe { self.view.owner() })
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToPubkey for Signer<'info> {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}
