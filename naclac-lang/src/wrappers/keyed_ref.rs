//! # Keyed Reference Wrappers
//!
//! Provides the `KeyedRef` and `KeyedRefMut` structs which act as lightweight wrappers 
//! pairing an `AccountInfo` with a typed reference to its deserialized (or zero-copy) data.
//! This allows easy access to both the account's structural metadata (like the pubkey) 
//! and its underlying data fields simultaneously.

use core::ops::{Deref, DerefMut};
use crate::prelude::{AccountInfo, ToAccountInfo, Result, Pubkey};

pub struct KeyedRef<'info, T> {
    pub info: AccountInfo<'info>,
    pub data: &'info T,
}

impl<'info, T> KeyedRef<'info, T> {
    pub fn new(info: AccountInfo<'info>, data: &'info T) -> Self {
        Self { info, data }
    }

    pub fn key(&self) -> Pubkey {
        crate::prelude::Key::key(&self.info)
    }

    pub fn load(&self) -> Result<&Self> {
        Ok(self)
    }
}

impl<'info, T> crate::prelude::Key for KeyedRef<'info, T> {
    fn key(&self) -> Pubkey {
        crate::prelude::Key::key(&self.info)
    }
}

impl<'info, T> ToAccountInfo<'info> for KeyedRef<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T> crate::prelude::ToAccountInfos<'info> for KeyedRef<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<'info, T> Deref for KeyedRef<'info, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

pub struct KeyedRefMut<'info, T> {
    pub info: AccountInfo<'info>,
    pub data: &'info mut T,
}

impl<'info, T> KeyedRefMut<'info, T> {
    pub fn new(info: AccountInfo<'info>, data: &'info mut T) -> Self {
        Self { info, data }
    }

    pub fn key(&self) -> Pubkey {
        crate::prelude::Key::key(&self.info)
    }

    pub fn load(&self) -> Result<&Self> {
        Ok(self)
    }

    pub fn load_mut(&mut self) -> Result<&mut Self> {
        Ok(self)
    }
}

impl<'info, T> crate::prelude::Key for KeyedRefMut<'info, T> {
    fn key(&self) -> Pubkey {
        crate::prelude::Key::key(&self.info)
    }
}

impl<'info, T> ToAccountInfo<'info> for KeyedRefMut<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T> crate::prelude::ToAccountInfos<'info> for KeyedRefMut<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<'info, T> Deref for KeyedRefMut<'info, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'info, T> DerefMut for KeyedRefMut<'info, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'info, T> crate::prelude::Owner for KeyedRef<'info, T> {
    fn owner(&self) -> Pubkey {
        crate::prelude::Owner::owner(&self.info)
    }
}

impl<'info, T> crate::prelude::Owner for KeyedRefMut<'info, T> {
    fn owner(&self) -> Pubkey {
        crate::prelude::Owner::owner(&self.info)
    }
}
