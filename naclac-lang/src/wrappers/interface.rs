//! # Interface Wrappers
//!
//! Provides generic program interface wrappers (Interface, TokenInterface) for 
//! dynamic program resolution.

use crate::prelude::AccountInfo;
use crate::prelude::{Pubkey, NaclacError, Result};
#[cfg(feature = "pinocchio")]
use crate::prelude::AccountView;
use crate::wrappers::{ToAccountInfo, ToPubkey};
use core::ops::Deref;

/// Wrapper for executable program accounts that adhere to an interface with multiple possible IDs.
#[derive(Clone)]
pub struct Interface<'info, T: Clone = ()> {
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

/// Unit struct representing the Token Program Interface (accepts standard Token or Token-2022).
#[derive(Clone, Copy)]
pub struct TokenInterface;

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> Interface<'info, T> {
    pub fn try_from(
        info: &AccountInfo<'info>,
        allowed_ids: &[Pubkey],
        index: usize,
    ) -> Result<Self> {
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
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::Key for Interface<'info, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::Owner for Interface<'info, T> {
    fn owner(&self) -> Pubkey {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> Deref for Interface<'info, T> {
    type Target = AccountInfo<'info>;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> ToAccountInfo<'info> for Interface<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> crate::wrappers::ToAccountInfos<'info> for Interface<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: Clone> ToPubkey for Interface<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        *self.info.key
    }
}

// --- PINOCCHIO BACKEND ---
#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> Interface<'info, T> {
    pub fn try_from(
        info: &AccountView,
        allowed_ids: &[Pubkey],
        index: usize,
    ) -> Result<Self> {
        if !info.executable() {
            return Err(NaclacError::ConstraintExecutable.err(index));
        }
        let mut matched = false;
        for expected_id in allowed_ids {
            if info.address() == expected_id.as_address() {
                matched = true;
                break;
            }
        }
        
        if !matched {
            return Err(NaclacError::ProgramIdMismatch.err(index));
        }
        Ok(Self { view: info.clone(), index, _phantom: core::marker::PhantomData })
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::Key for Interface<'info, T> {
    fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::Owner for Interface<'info, T> {
    fn owner(&self) -> Pubkey {
        // SAFETY: We are guaranteed the AccountView is initialized directly by the runtime.
        Pubkey::from_address(unsafe { self.view.owner() })
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> Deref for Interface<'info, T> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> crate::wrappers::ToAccountView for Interface<'info, T> {
    fn to_account_view(&self) -> AccountView {
        self.view.clone()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> ToAccountInfo<'info> for Interface<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        AccountInfo {
            view: self.view.clone(),
            _phantom: core::marker::PhantomData,
        }
    }
}


#[cfg(feature = "pinocchio")]
impl<'info, T: Clone> ToPubkey for Interface<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }
}
