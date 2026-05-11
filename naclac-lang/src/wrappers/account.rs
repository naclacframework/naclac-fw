//! # Borsh Account Wrapper
//!
//! Provides the `Account` wrapper struct exclusively used in the `solana` execution backend.
//! It handles the dynamic heap-based deserialization of Borsh structures.

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::{AccountInfo, Pubkey, NaclacError, Result, BorshDeserialize, BorshSerialize};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::{ToAccountInfo, ToPubkey};
#[cfg(not(feature = "pinocchio"))]
use core::ops::{Deref, DerefMut};

/// Wrapper for Borsh-serialized accounts (The "Borsh Aspect").
/// 
/// This struct is used in standard Solana programs to handle automatic
/// deserialization and serialization of account data using Borsh.
/// 
/// naclac uses an 8-byte discriminator at the start of the account data
/// for compatibility with Anchor and other standard tools.
#[cfg(not(feature = "pinocchio"))]
#[derive(Clone)]
pub struct Account<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> {
    pub info: AccountInfo<'info>,
    pub data: T,
    pub index: usize,
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Account<'info, T> {
    /// Deserializes the account data into the inner type, skipping the 8-byte discriminator.
    pub fn try_from(info: &AccountInfo<'info>, index: usize) -> Result<Self> {
        let data = info.try_borrow_data()?;
        if data.len() < T::discriminator_len() {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }

        // Verify discriminator
        // (In a real implementation, we would compare with the expected discriminator)
        
        let mut reader = &data[T::discriminator_len()..];
        let data = T::deserialize(&mut reader).map_err(|_| NaclacError::DeserializationFailed.err(index))?;
        
        Ok(Self {
            info: info.clone(),
            data,
            index,
        })
    }

    /// Serializes the inner type back into the account data, skipping the 8-byte discriminator.
    pub fn exit(&self) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let mut writer = &mut data[T::discriminator_len()..];
        self.data.serialize(&mut writer).map_err(|_| NaclacError::SerializationFailed.err(self.index))?;
        Ok(())
    }

    pub fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::Key for Account<'info, T> {
    fn key(&self) -> Pubkey {
        self.key()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::Owner for Account<'info, T> {
    fn owner(&self) -> Pubkey {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref for Account<'info, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut for Account<'info, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo<'info> for Account<'info, T> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::ToAccountInfos<'info> for Account<'info, T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToPubkey for Account<'info, T> {
    fn to_pubkey(&self) -> Pubkey {
        self.key()
    }
}
