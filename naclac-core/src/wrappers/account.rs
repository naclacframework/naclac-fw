//! # Borsh Account Wrapper
//!
//! Provides the `Account` wrapper struct exclusively used in the `solana` execution backend.
//! It handles the dynamic heap-based deserialization of Borsh structures.

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::{AccountInfo, Address, BorshDeserialize, BorshSerialize, NaclacError, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::{ToAccountInfo, ToAddress};
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
pub struct Account<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> {
    pub info: AccountInfo,
    pub data: T,
    pub index: usize,
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Account<T> {
    /// Deserializes the account data into the inner type, validating the 8-byte
    /// discriminator before skipping it. Without this check, two different
    /// `#[component]` types owned by the same program are indistinguishable to
    /// this wrapper — a same-program account of the wrong type would silently
    /// be reinterpreted as `T` via raw Borsh deserialization.
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        let data = info.try_borrow_data()?;
        let disc_len = T::discriminator_len();
        if data.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if data[..disc_len] != T::DISCRIMINATOR[..disc_len] {
            return Err(NaclacError::AccountNotInitialized.err(index));
        }

        let mut reader = &data[disc_len..];
        let data = T::deserialize(&mut reader)
            .map_err(|_| NaclacError::DeserializationFailed.err(index))?;

        Ok(Self {
            info: info.clone(),
            data,
            index,
        })
    }

    /// On the non-pinocchio (Borsh) path, `try_from_mut` is identical to `try_from`.
    /// The discriminator check savings only apply in the pinocchio zero-copy path.
    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    /// Serializes the inner type back into the account data, (re-)writing the
    /// 8-byte discriminator before it. Re-stamping the discriminator on every
    /// exit keeps this symmetric with `try_from`'s validation and matches the
    /// macro-generated `try_serialize` for `#[component]` Borsh types.
    pub fn exit(&self) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let disc_len = T::discriminator_len();
        data[..disc_len].copy_from_slice(&T::DISCRIMINATOR[..disc_len]);
        let mut writer = &mut data[disc_len..];
        self.data
            .serialize(&mut writer)
            .map_err(|_| NaclacError::SerializationFailed.err(self.index))?;
        Ok(())
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::Owner
    for Account<T>
{
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref for Account<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut
    for Account<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo
    for Account<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::ToAccountInfos for Account<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAddress
    for Account<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Account<T> {
    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        self.info.sub_lamports(amount)
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        self.info.add_lamports(amount)
    }

    pub fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::AsRefByteSlice for Account<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::NaclacAccount for Account<T>
{
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    #[inline(always)]
    fn try_from_mut(info: &crate::prelude::AccountType, index: usize) -> Result<Self> {
        Self::try_from_mut(info, index)
    }

    #[inline(always)]
    fn exit(&self, program_id: &crate::prelude::Address) -> Result<()> {
        if *self.info.owner == *program_id {
            self.exit()
        } else {
            Ok(())
        }
    }
}
