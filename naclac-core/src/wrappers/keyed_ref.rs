//! # Keyed Reference Wrappers
//!
//! Provides the `KeyedRef` and `KeyedRefMut` structs which act as lightweight wrappers
//! pairing an `AccountInfo` with a typed reference to its deserialized (or zero-copy) data.
//! This allows easy access to both the account's structural metadata (like the address)
//! and its underlying data fields simultaneously.

use crate::prelude::{AccountInfo, Address, Result, ToAccountInfo};
use core::ops::{Deref, DerefMut};

pub struct KeyedRef<T: 'static> {
    pub info: AccountInfo,
    pub data: &'static T,
}

impl<T: 'static> KeyedRef<T> {
    pub fn new(info: AccountInfo, data: &'static T) -> Self {
        Self { info, data }
    }

    pub fn address(&self) -> Address {
        self.info.address()
    }

    pub fn load(&self) -> Result<&Self> {
        Ok(self)
    }
}

impl<T: 'static> ToAccountInfo for KeyedRef<T> {
    fn to_account_info(&self) -> AccountInfo {
        #[cfg(feature = "pinocchio")]
        {
            self.info
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            self.info.clone()
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: 'static> crate::prelude::ToAccountInfos for KeyedRef<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<T: 'static> Deref for KeyedRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

pub struct KeyedRefMut<T: 'static> {
    pub info: AccountInfo,
    pub data: &'static mut T,
}

impl<T: 'static> KeyedRefMut<T> {
    pub fn new(info: AccountInfo, data: &'static mut T) -> Self {
        Self { info, data }
    }

    pub fn address(&self) -> Address {
        self.info.address()
    }

    pub fn load(&self) -> Result<&Self> {
        Ok(self)
    }

    pub fn load_mut(&mut self) -> Result<&mut Self> {
        Ok(self)
    }
}

impl<T: 'static> ToAccountInfo for KeyedRefMut<T> {
    fn to_account_info(&self) -> AccountInfo {
        #[cfg(feature = "pinocchio")]
        {
            self.info
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            self.info.clone()
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: 'static> crate::prelude::ToAccountInfos for KeyedRefMut<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<T: 'static> Deref for KeyedRefMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: 'static> DerefMut for KeyedRefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> crate::prelude::Owner for KeyedRef<T> {
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

impl<T> crate::prelude::Owner for KeyedRefMut<T> {
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}
