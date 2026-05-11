//! # Zero-Copy Account Loader
//!
//! Provides the AccountLoader wrapper for hydrating and interacting with #[component(zero_copy)] 
//! accounts. It safely manages the raw SBF pointers without allocations.

use crate::prelude::{NaclacError, Pubkey, AccountInfo};
use crate::wrappers::{Key, Owner, ToAccountInfo};
use core::ops::{Deref, DerefMut};

// --- SHARED LOGIC ---

/// Trait for types that can be zero-copy loaded from raw bytes.
pub trait NaclacZeroCopy:
    bytemuck::Pod + bytemuck::Zeroable + crate::wrappers::Discriminator
{
    #[inline(always)]
    fn load(data: &[u8]) -> crate::prelude::Result<&Self> {
        if data.len() < core::mem::size_of::<Self>() {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        Ok(bytemuck::from_bytes(&data[..core::mem::size_of::<Self>()]))
    }

    #[inline(always)]
    fn load_mut(data: &mut [u8]) -> crate::prelude::Result<&mut Self> {
        if data.len() < core::mem::size_of::<Self>() {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        Ok(bytemuck::from_bytes_mut(
            &mut data[..core::mem::size_of::<Self>()],
        ))
    }
}

/// Wrapper for Zero-Copy accounts (The "Zero-Copy Aspect").
///
/// This provides extreme performance by allowing direct access to the account
/// data in the VM memory without deserialization.
#[derive(Clone)]
pub struct AccountLoader<'info, T: NaclacZeroCopy> {
    pub info: crate::prelude::AccountInfo<'info>,
    pub index: usize,
    pub _phantom: core::marker::PhantomData<(&'info (), T)>,
}

impl<'info, T: NaclacZeroCopy> AccountLoader<'info, T> {
    pub fn try_from(
        info: &crate::prelude::AccountInfo<'info>,
        index: usize,
    ) -> crate::prelude::Result<Self> {
        Ok(Self {
            info: info.clone(),
            index,
            _phantom: core::marker::PhantomData,
        })
    }

    pub fn key(&self) -> Pubkey {
        crate::wrappers::Key::key(&self.info)
    }

    /// Loads the account data for reading.
    #[cfg(not(feature = "pinocchio"))]
    pub fn load(&self) -> crate::prelude::Result<core::cell::Ref<'_, T>> {
        let data = self.info.try_borrow_data()?;
        core::cell::Ref::filter_map(data, |d| T::load(d).ok())
            .map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))
    }

    /// Loads the account data for reading.
    #[cfg(feature = "pinocchio")]
    pub fn load(&self) -> crate::prelude::Result<&T> {
        let data = self.info.data();
        T::load(data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))
    }

    /// Loads the account data for writing.
    #[cfg(not(feature = "pinocchio"))]
    pub fn load_mut(&self) -> crate::prelude::Result<core::cell::RefMut<'_, T>> {
        let data = self.info.try_borrow_mut_data()?;
        core::cell::RefMut::filter_map(data, |d| T::load_mut(d).ok())
            .map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))
    }

    /// Loads the account data for writing.
    #[cfg(feature = "pinocchio")]
    pub fn load_mut(&self) -> crate::prelude::Result<&mut T> {
        let data = self.info.data_mut();
        T::load_mut(data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))
    }

    /// Hydrates the account into a typed reference wrapper.
    pub fn hydrate(
        &self,
    ) -> crate::prelude::Result<crate::wrappers::keyed_ref::KeyedRef<'info, T>> {
        // SAFETY: We transmute the lifetime to tie it to the 'info of the AccountLoader.
        let data: &'info [u8] = unsafe {
            #[cfg(not(feature = "pinocchio"))]
            {
                let borrow = self.info.try_borrow_data()?;
                core::slice::from_raw_parts(borrow.as_ptr(), borrow.len())
            }
            #[cfg(feature = "pinocchio")]
            {
                core::slice::from_raw_parts(self.info.data().as_ptr(), self.info.data().len())
            }
        };

        if data.is_empty() {
            return Err(NaclacError::AccountNotInitialized.err(self.index));
        }

        let data_ref =
            T::load(data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))?;

        Ok(crate::wrappers::keyed_ref::KeyedRef::new(
            self.info.clone(),
            data_ref,
        ))
    }

    /// Hydrates the account into a mutable typed reference wrapper.
    pub fn hydrate_mut(
        &self,
    ) -> crate::prelude::Result<crate::wrappers::keyed_ref::KeyedRefMut<'info, T>> {
        // SAFETY: We transmute the mutable lifetime to tie it to the 'info of the AccountLoader.
        let data: &'info mut [u8] = unsafe {
            #[cfg(not(feature = "pinocchio"))]
            {
                let mut borrow = self.info.try_borrow_mut_data()?;
                core::slice::from_raw_parts_mut(borrow.as_mut_ptr(), borrow.len())
            }
            #[cfg(feature = "pinocchio")]
            {
                core::slice::from_raw_parts_mut(
                    self.info.data_mut().as_mut_ptr(),
                    self.info.data_mut().len(),
                )
            }
        };

        if data.is_empty() {
            return Err(NaclacError::AccountNotInitialized.err(self.index));
        }

        let data_ref = T::load_mut(data)
            .map_err(|_| NaclacError::InvalidAccountDiscriminator.err(self.index))?;

        Ok(crate::wrappers::keyed_ref::KeyedRefMut::new(
            self.info.clone(),
            data_ref,
        ))
    }
}

impl<'info, T: NaclacZeroCopy> Deref for AccountLoader<'info, T> {
    type Target = crate::prelude::AccountInfo<'info>;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl<'info, T: NaclacZeroCopy> DerefMut for AccountLoader<'info, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl<'info, T: NaclacZeroCopy> ToAccountInfo<'info> for AccountLoader<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: crate::prelude::BorshDeserialize + crate::prelude::Discriminator + crate::wrappers::account_loader::NaclacZeroCopy> crate::wrappers::ToAccountInfos<'info> for AccountLoader<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<'info, T: NaclacZeroCopy> Key for AccountLoader<'info, T> {
    fn key(&self) -> Pubkey {
        self.info.key()
    }
}

impl<'info, T: NaclacZeroCopy> Owner for AccountLoader<'info, T> {
    fn owner(&self) -> Pubkey {
        self.info.owner()
    }
}

impl<'info, T: NaclacZeroCopy> crate::wrappers::ToPubkey for AccountLoader<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        self.info.key()
    }
}
