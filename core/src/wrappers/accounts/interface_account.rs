// ===========================================================================
// wrappers/accounts/interface_account.rs — InterfaceAccount<T>
// ===========================================================================

//! `InterfaceAccount<T>` accepts an account owned by any of the programs
//! `T` lists in `ValidateInterfaceLayout::OWNERS` (naclac-token's
//! `TokenAccount`/`Mint`: Token or Token-2022), regardless of what `T`'s own
//! `Discriminator::validate_account` says (e.g. `TokenAccount`/`Mint`'s own
//! `validate_account` is strict — Token-program-only). This can't be done
//! via a type alias to `Account<T>` (an alias carries no behavior of its own
//! to override `T`'s existing `validate_account` with — `InterfaceAccount<Mint>`
//! would just *be* `Account<Mint>`, identically strict). So this is a
//! genuinely separate wrapper, structurally mirroring `Account<T>`
//! (`accounts/account.rs`) almost exactly — same cfg-routed Borsh-vs-zero-copy
//! split, same set of trait impls — except `try_from`/`try_from_mut` check
//! the owner directly instead of calling `T::validate_account`.

// ===========================================================================
// Borsh branch — owns a deserialized copy of T
// ===========================================================================

#[cfg(feature = "borsh")]
use crate::prelude::{
    AccountInfo, Address, BorshDeserialize, BorshSerialize, NaclacError, Owner, Result,
};
#[cfg(feature = "borsh")]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(feature = "borsh")]
use core::ops::{Deref, DerefMut};

#[cfg(feature = "borsh")]
fn check_interface_owner<T: crate::wrappers::ValidateInterfaceLayout>(
    info: &AccountInfo,
    index: usize,
) -> core::result::Result<(), NaclacError> {
    let owner = Owner::program_owner(info);
    if !T::OWNERS.contains(&owner) {
        return Err(NaclacError::ConstraintOwner);
    }
    let _ = index;
    Ok(())
}

/// Borsh-backed `InterfaceAccount<T>` — see module doc for why this exists
/// alongside `Account<T>` rather than as an alias to it. Field shape and
/// every method here mirror `Account<T>`'s Borsh branch exactly, except
/// `try_from`/`try_from_mut` check the owner directly instead of calling
/// `T::validate_account`.
#[cfg(feature = "borsh")]
#[derive(Clone)]
pub struct InterfaceAccount<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> {
    pub info: AccountInfo,
    pub data: T,
    // Deliberately private — see `accounts/account.rs`'s `Account<T>` doc
    // comment on its own `index` field for why.
    index: usize,
}

#[cfg(feature = "borsh")]
impl<
        T: BorshDeserialize
            + BorshSerialize
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > InterfaceAccount<T>
{
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        check_interface_owner::<T>(info, index).map_err(|e| e.err(index))?;
        let raw = info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if &raw[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(index))?;

        let mut reader = &raw[disc_len..];
        let data = T::deserialize(&mut reader).map_err(|error| {
            super::discard_error(error);
            NaclacError::DeserializationFailed.err(index)
        })?;

        Ok(Self {
            info: *info,
            data,
            index,
        })
    }

    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    /// Re-borrows `self.info`'s current data and re-deserializes it into
    /// `self.data`, discarding the cached copy taken at construction time.
    /// Needed after any CPI that mutates this account — this Borsh branch
    /// deserializes once up front and never re-reads on its own, so a plain
    /// field access here (unlike the zero-copy branch, whose accessors read
    /// live memory directly) would otherwise keep returning the pre-CPI
    /// value. Re-validates ownership for the same reason `try_from` does:
    /// the CPI this is called after could itself have closed or reassigned
    /// the account.
    pub fn reload(&mut self) -> Result<()> {
        check_interface_owner::<T>(&self.info, self.index).map_err(|e| e.err(self.index))?;
        let raw = self.info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(self.index));
        }
        if &raw[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(self.index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(self.index))?;

        let mut reader = &raw[disc_len..];
        self.data = T::deserialize(&mut reader).map_err(|error| {
            super::discard_error(error);
            NaclacError::DeserializationFailed.err(self.index)
        })?;
        Ok(())
    }

    pub fn exit(&self) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        let (discriminator, mut writer) = data
            .split_at_mut_checked(disc_len)
            .ok_or_else(|| NaclacError::AccountDataTooSmall.err(self.index))?;
        discriminator.copy_from_slice(T::DISCRIMINATOR);
        self.data.serialize(&mut writer).map_err(|error| {
            super::discard_error(error);
            NaclacError::SerializationFailed.err(self.index)
        })?;
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::Owner
    for InterfaceAccount<T>
{
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref
    for InterfaceAccount<T>
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut
    for InterfaceAccount<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo
    for InterfaceAccount<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::ToAccountInfos for InterfaceAccount<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info]
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAddress
    for InterfaceAccount<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> InterfaceAccount<T> {
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

#[cfg(feature = "borsh")]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandle<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(feature = "borsh")]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::AsRefByteSlice for InterfaceAccount<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

#[cfg(feature = "borsh")]
impl<
        T: BorshDeserialize
            + BorshSerialize
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > crate::wrappers::NaclacAccount for InterfaceAccount<T>
{
    const PROGRAM_OWNED: bool = <T as crate::wrappers::Discriminator>::PROGRAM_OWNED;
    const OWNER_VALIDATED: bool = true;

    #[inline(always)]
    fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    #[inline(always)]
    fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
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

// ===========================================================================
// Wincode branch
// ===========================================================================

#[cfg(feature = "wincode")]
use crate::prelude::{AccountInfo, Address, NaclacError, Owner, Result};
#[cfg(feature = "wincode")]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(feature = "wincode")]
use core::ops::{Deref, DerefMut};

#[cfg(feature = "wincode")]
fn check_interface_owner_wincode<T: crate::wrappers::ValidateInterfaceLayout>(
    info: &AccountInfo,
    index: usize,
) -> core::result::Result<(), NaclacError> {
    let owner = Owner::program_owner(info);
    if !T::OWNERS.contains(&owner) {
        return Err(NaclacError::ConstraintOwner);
    }
    let _ = index;
    Ok(())
}

#[cfg(feature = "wincode")]
#[derive(Clone)]
pub struct InterfaceAccount<
    T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
        + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
        + crate::wrappers::Discriminator,
> {
    pub info: AccountInfo,
    pub data: T,
    index: usize,
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > InterfaceAccount<T>
{
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        check_interface_owner_wincode::<T>(info, index).map_err(|e| e.err(index))?;
        let raw = info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if &raw[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(index))?;

        let mut reader = &raw[disc_len..];
        let data = crate::prelude::wincode::config::deserialize_from(
            &mut reader,
            crate::wincode_config::CONFIG,
        )
        .map_err(|_| NaclacError::DeserializationFailed.err(index))?;

        Ok(Self {
            info: *info,
            data,
            index,
        })
    }

    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    pub fn reload(&mut self) -> Result<()> {
        check_interface_owner_wincode::<T>(&self.info, self.index)
            .map_err(|e| e.err(self.index))?;
        let raw = self.info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(self.index));
        }
        if &raw[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(self.index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(self.index))?;

        let mut reader = &raw[disc_len..];
        self.data = crate::prelude::wincode::config::deserialize_from(
            &mut reader,
            crate::wincode_config::CONFIG,
        )
        .map_err(|_| NaclacError::DeserializationFailed.err(self.index))?;
        Ok(())
    }

    pub fn exit(&self) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        let (discriminator, mut writer) = data
            .split_at_mut_checked(disc_len)
            .ok_or_else(|| NaclacError::AccountDataTooSmall.err(self.index))?;
        discriminator.copy_from_slice(T::DISCRIMINATOR);
        crate::prelude::wincode::config::serialize_into(
            &mut writer,
            &self.data,
            crate::wincode_config::CONFIG,
        )
        .map_err(|_| NaclacError::SerializationFailed.err(self.index))?;
        Ok(())
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::Owner for InterfaceAccount<T>
{
    fn program_owner(&self) -> Address {
        crate::wrappers::Owner::program_owner(&self.info)
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > Deref for InterfaceAccount<T>
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > DerefMut for InterfaceAccount<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > ToAccountInfo for InterfaceAccount<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info
    }
}

#[cfg(all(feature = "wincode", not(feature = "pinocchio")))]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::ToAccountInfos for InterfaceAccount<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info]
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > ToAddress for InterfaceAccount<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > InterfaceAccount<T>
{
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

#[cfg(feature = "wincode")]
impl<
        'a,
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::cpi_handle::ToCpiHandle<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(feature = "wincode")]
impl<
        'a,
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::AsRefByteSlice for InterfaceAccount<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        #[cfg(feature = "pinocchio")]
        {
            self.info.view.address().as_ref()
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            self.info.key.as_ref()
        }
    }
}

#[cfg(feature = "wincode")]
impl<
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > crate::wrappers::NaclacAccount for InterfaceAccount<T>
{
    const PROGRAM_OWNED: bool = <T as crate::wrappers::Discriminator>::PROGRAM_OWNED;
    const OWNER_VALIDATED: bool = true;

    #[inline(always)]
    fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    #[inline(always)]
    fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from_mut(info, index)
    }

    #[inline(always)]
    fn exit(&self, program_id: &crate::prelude::Address) -> Result<()> {
        if crate::wrappers::Owner::program_owner(&self.info) == *program_id {
            self.exit()
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Zero-copy branch — caches a raw pointer into the account's own buffer
// ===========================================================================

#[cfg(feature = "bytemuck")]
use crate::prelude::{AccountInfo, Address, NaclacError, Owner};
#[cfg(feature = "bytemuck")]
use crate::wrappers::accounts::NaclacZeroCopy;
#[cfg(feature = "bytemuck")]
use crate::wrappers::ToAccountInfo;
#[cfg(feature = "bytemuck")]
use core::marker::PhantomData;
#[cfg(feature = "bytemuck")]
use core::ops::{Deref, DerefMut};

#[cfg(feature = "bytemuck")]
fn check_interface_owner_zc<T: crate::wrappers::ValidateInterfaceLayout>(
    info: &AccountInfo,
    index: usize,
) -> core::result::Result<(), NaclacError> {
    let owner = Owner::program_owner(info);
    if !T::OWNERS.contains(&owner) {
        return Err(NaclacError::ConstraintOwner);
    }
    let _ = index;
    Ok(())
}

/// Zero-copy-backed `InterfaceAccount<T>` — see module doc. Field shape and
/// every method here mirror `Account<T>`'s zero-copy branch
/// (`accounts/account.rs`) exactly, except `try_from`/`try_from_mut` check
/// the owner directly instead of calling `T::validate_account`.
#[cfg(feature = "bytemuck")]
pub struct InterfaceAccount<T: NaclacZeroCopy> {
    pub info: AccountInfo,
    header_ptr: *mut T,
    // Deliberately private — see `accounts/account.rs`'s `Account<T>` doc
    // comment on its own `index` field for why.
    index: usize,
    pub _phantom: PhantomData<T>,
}

#[cfg(feature = "bytemuck")]
unsafe impl<T: NaclacZeroCopy + Send> Send for InterfaceAccount<T> {}
#[cfg(feature = "bytemuck")]
unsafe impl<T: NaclacZeroCopy + Sync> Sync for InterfaceAccount<T> {}

#[cfg(all(feature = "bytemuck", feature = "pinocchio"))]
impl<T: NaclacZeroCopy> Clone for InterfaceAccount<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(all(feature = "bytemuck", not(feature = "pinocchio")))]
impl<T: NaclacZeroCopy> Clone for InterfaceAccount<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            info: self.info,
            header_ptr: self.header_ptr,
            index: self.index,
            _phantom: PhantomData,
        }
    }
}

#[cfg(all(feature = "bytemuck", feature = "pinocchio"))]
impl<T: NaclacZeroCopy> Copy for InterfaceAccount<T> {}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy + crate::wrappers::ValidateInterfaceLayout> InterfaceAccount<T> {
    pub fn try_from(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        check_interface_owner_zc::<T>(info, index).map_err(|e| e.err(index))?;
        let header_ptr: *mut T;
        let disc_len = T::DISCRIMINATOR.len();

        #[cfg(not(feature = "pinocchio"))]
        {
            let data = info.try_borrow_data()?;
            let _: &T =
                T::load(&data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(index))?;
            T::validate_interface_layout(&data[disc_len..]).map_err(|e| e.err(index))?;
            // SAFETY: same as `Account<T>`'s zero-copy `try_from` — the SBF
            // runtime guarantees account data is valid for the whole
            // instruction and does not move it.
            header_ptr = unsafe { (data.as_ptr() as *mut u8).add(disc_len) as *mut T };
        }

        #[cfg(feature = "pinocchio")]
        {
            let data = info.data();
            let _: &T =
                T::load(data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(index))?;
            T::validate_interface_layout(&data[disc_len..]).map_err(|e| e.err(index))?;
            // SAFETY: same as `Account<T>`'s zero-copy `try_from`.
            header_ptr = unsafe { info.view.data_ptr().add(disc_len) as *mut T };
        }

        Ok(Self {
            info: *info,
            header_ptr,
            index,
            _phantom: PhantomData,
        })
    }

    /// Alias for `try_from`, kept for symmetry with `Account<T>::try_from_mut`
    /// and with `init` accounts. Previously validated size and layout but
    /// never called `T::load`, so it skipped the discriminator check
    /// entirely — a `mut` slot with no `seeds`/`address` constraint accepted
    /// any same-owner account of the wrong type. See `Account::try_from_mut`'s
    /// doc comment for why "verified earlier" doesn't hold for a `mut` field.
    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        Self::try_from(info, index)
    }

    /// Re-validates ownership of `self.info`. `header_ptr` already points
    /// directly into the account's own live data buffer, so unlike the
    /// Borsh branch's `reload`, there is no cached copy to refresh here —
    /// every field access already observes the current bytes. What isn't
    /// automatically re-checked is *ownership*: a CPI could have closed or
    /// reassigned this account since construction, which wouldn't fault a
    /// raw read through `header_ptr` but would mean the bytes it reads no
    /// longer mean what `T` expects them to. Call this after any CPI where
    /// that's a real possibility, mirroring the Borsh branch's own
    /// re-validation for the same reason.
    pub fn reload(&mut self) -> crate::prelude::Result<()> {
        check_interface_owner_zc::<T>(&self.info, self.index).map_err(|e| e.err(self.index))
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Deref for InterfaceAccount<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: header_ptr was validated at construction time (try_from) and the
        // SBF runtime guarantees account data does not move during instruction execution.
        unsafe { &*self.header_ptr }
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> DerefMut for InterfaceAccount<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: same as Deref.
        unsafe { &mut *self.header_ptr }
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> ToAccountInfo for InterfaceAccount<T> {
    #[inline(always)]
    fn to_account_info(&self) -> AccountInfo {
        self.info
    }
}

#[cfg(all(feature = "bytemuck", not(feature = "pinocchio")))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAccountInfos for InterfaceAccount<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info]
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Owner for InterfaceAccount<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> crate::wrappers::ToAddress for InterfaceAccount<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> InterfaceAccount<T> {
    pub fn sub_lamports(&self, amount: u64) -> crate::prelude::Result<()> {
        self.info.sub_lamports(amount)
    }

    pub fn add_lamports(&self, amount: u64) -> crate::prelude::Result<()> {
        self.info.add_lamports(amount)
    }

    pub fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "bytemuck")]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandle<'a> for InterfaceAccount<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandleMut<'a>
    for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> crate::wrappers::AsRefByteSlice for InterfaceAccount<T> {
    fn as_ref_byte_slice(&self) -> &[u8] {
        #[cfg(feature = "pinocchio")]
        {
            self.info.view.address().as_ref()
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            self.info.key.as_ref()
        }
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy + crate::wrappers::ValidateInterfaceLayout> crate::wrappers::NaclacAccount
    for InterfaceAccount<T>
{
    const PROGRAM_OWNED: bool = <T as crate::wrappers::Discriminator>::PROGRAM_OWNED;
    const OWNER_VALIDATED: bool = true;

    #[inline(always)]
    fn try_from(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        Self::try_from(info, index)
    }

    #[inline(always)]
    fn try_from_mut(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        Self::try_from_mut(info, index)
    }
}
