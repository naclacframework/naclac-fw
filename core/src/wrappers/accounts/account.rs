// ===========================================================================
// wrappers/accounts/account.rs — the unified Account<T>
// ===========================================================================

//! `Account<T>` is naclac's one account-wrapper name. Internally it
//! cfg-routes between two structurally different implementations:
//!
//! - `all(borsh, not(pinocchio))`: owns a Borsh-deserialized copy of `T`.
//! - `any(pinocchio, not(borsh))`: caches a raw pointer directly into the
//!   account's own data buffer (zero-copy).
//!
//! These two conditions are mutually exclusive and exhaustive over every
//! `(pinocchio, borsh)` combination (verified in
//! `core/docs/accounts-consolidation-plan.md`'s truth table), so
//! exactly one `Account<T>` definition is ever compiled.

// ===========================================================================
// Borsh branch — owns a deserialized copy of T
// ===========================================================================

#[cfg(feature = "borsh")]
use crate::prelude::{AccountInfo, Address, BorshDeserialize, BorshSerialize, NaclacError, Result};
#[cfg(feature = "borsh")]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(feature = "borsh")]
use core::ops::{Deref, DerefMut};

/// Wrapper for Borsh-serialized accounts (The "Borsh Aspect").
///
/// This struct is used in standard Solana programs to handle automatic
/// deserialization and serialization of account data using Borsh.
///
/// The account data starts with `T::DISCRIMINATOR` (8 bytes unless the
/// component declares a custom one).
#[cfg(feature = "borsh")]
#[derive(Clone)]
pub struct Account<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> {
    pub info: AccountInfo,
    pub data: T,
    // Deliberately private — a `pub` field here would sit in the same
    // field-resolution namespace as `T`'s own fields (reached only via
    // `Deref`), so any component field sharing this name would be silently
    // shadowed by this bookkeeping field instead (Rust resolves the outer
    // type's own fields before falling through to `Deref`).
    index: usize,
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Account<T> {
    /// Deserializes the account data into the inner type, validating the
    /// discriminator before skipping it. Without this check, two different
    /// `#[component]` types owned by the same program are indistinguishable to
    /// this wrapper — a same-program account of the wrong type would silently
    /// be reinterpreted as `T` via raw Borsh deserialization.
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        T::validate_account(info, index)?;
        let data = info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if data.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if &data[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(index));
        }

        let mut reader = &data[disc_len..];
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

    /// On the non-pinocchio (Borsh) path, `try_from_mut` is identical to `try_from`.
    /// The discriminator check savings only apply in the pinocchio zero-copy path.
    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> Result<Self> {
        Self::try_from(info, index)
    }

    /// Serializes the inner type back into the account data, (re-)writing the
    /// discriminator before it. Re-stamping the discriminator on every
    /// exit keeps this symmetric with `try_from`'s validation and matches the
    /// macro-generated `try_serialize` for `#[component]` Borsh types.
    pub fn exit(&self) -> Result<()> {
        if !self.data.within_max_len() {
            return Err(NaclacError::CapacityExceeded.err(self.index));
        }
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
    for Account<T>
{
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref for Account<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut
    for Account<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo
    for Account<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::ToAccountInfos for Account<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info]
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAddress
    for Account<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "borsh")]
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

#[cfg(feature = "borsh")]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(feature = "borsh")]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::AsRefByteSlice for Account<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

#[cfg(feature = "borsh")]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::NaclacAccount for Account<T>
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
// Wincode branch — owns a deserialized copy of T
// ===========================================================================

#[cfg(feature = "wincode")]
use crate::prelude::{AccountInfo, Address, NaclacError, Result};
#[cfg(feature = "wincode")]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(feature = "wincode")]
use core::ops::{Deref, DerefMut};

#[cfg(feature = "wincode")]
#[derive(Clone)]
pub struct Account<
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
            + crate::wrappers::Discriminator,
    > Account<T>
{
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        T::validate_account(info, index)?;
        let data = info.try_borrow_data()?;
        let disc_len = T::DISCRIMINATOR.len();
        if data.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if &data[..disc_len] != T::DISCRIMINATOR {
            return Err(NaclacError::InvalidAccountDiscriminator.err(index));
        }

        let mut reader = &data[disc_len..];
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

    pub fn exit(&self) -> Result<()> {
        if !self.data.within_max_len() {
            return Err(NaclacError::CapacityExceeded.err(self.index));
        }
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
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::Owner for Account<T>
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
    > Deref for Account<T>
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
    > DerefMut for Account<T>
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
    > ToAccountInfo for Account<T>
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
    > crate::wrappers::ToAccountInfos for Account<T>
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
    > ToAddress for Account<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "wincode")]
impl<
        'a,
        T: crate::prelude::wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T>
            + crate::prelude::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>
            + crate::wrappers::Discriminator,
    > crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T>
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
    > crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T>
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
    > crate::wrappers::AsRefByteSlice for Account<T>
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
            + crate::wrappers::Discriminator,
    > crate::wrappers::NaclacAccount for Account<T>
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
//
// At construction time (`try_from`), we validate the discriminator and size
// ONCE, then cache a raw `*mut T` pointer directly into the account data at
// offset `disc_len`. `Deref`/`DerefMut` then cost exactly one unsafe pointer
// dereference — no size check, no discriminator check, no bytemuck on every
// access. Mirrors Anchor's `Slab<H, HeaderOnly>` design.

#[cfg(feature = "bytemuck")]
use crate::prelude::{AccountInfo, Address, NaclacError};
#[cfg(feature = "bytemuck")]
use crate::wrappers::accounts::NaclacZeroCopy;
#[cfg(feature = "bytemuck")]
use crate::wrappers::{Owner, ToAccountInfo};
#[cfg(feature = "bytemuck")]
use core::marker::PhantomData;
#[cfg(feature = "bytemuck")]
use core::ops::{Deref, DerefMut};

/// Wrapper for Zero-Copy accounts.
///
/// Stores a cached raw pointer to the typed data `T` inside the account's data buffer
/// (at offset +8, past the discriminator). `Deref`/`DerefMut` cost a single pointer
/// dereference — no per-access validation overhead.
///
/// Mirrors Anchor's `Slab<H, HeaderOnly>` design for binary-size efficiency.
#[cfg(feature = "bytemuck")]
pub struct Account<T: NaclacZeroCopy> {
    /// The underlying account info (needed for owner(), to_account_info(), etc.)
    pub info: AccountInfo,
    /// Cached raw pointer into the account's data at offset `disc_len` (past
    /// any discriminator).
    ///
    /// SAFETY invariant: valid for the entire instruction execution lifetime.
    /// The SBF runtime guarantees that account data buffers don't move during an instruction.
    header_ptr: *mut T,
    pub _phantom: PhantomData<T>,
}

// SAFETY: `header_ptr` points into Solana account data. Solana's single-threaded SBF
// execution model means Send/Sync are both safe here.
#[cfg(feature = "bytemuck")]
unsafe impl<T: NaclacZeroCopy + Send> Send for Account<T> {}
#[cfg(feature = "bytemuck")]
unsafe impl<T: NaclacZeroCopy + Sync> Sync for Account<T> {}

// Manual Clone: raw pointers are Copy so cloning the struct is fine.
#[cfg(all(feature = "bytemuck", feature = "pinocchio"))]
impl<T: NaclacZeroCopy> Clone for Account<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(all(feature = "bytemuck", not(feature = "pinocchio")))]
impl<T: NaclacZeroCopy> Clone for Account<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            info: self.info,
            header_ptr: self.header_ptr,
            _phantom: PhantomData,
        }
    }
}

#[cfg(all(feature = "bytemuck", feature = "pinocchio"))]
impl<T: NaclacZeroCopy> Copy for Account<T> {}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Account<T> {
    /// Construct from an `AccountInfo`, validating the discriminator and size once.
    ///
    /// On success the cached `header_ptr` is set to `data_ptr + disc_len`, pointing
    /// directly at `T` inside the account buffer. All subsequent `Deref`/`DerefMut`
    /// access is a single pointer load with no further checks.
    pub fn try_from(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        // Validate discriminator + size, then cache the raw data pointer.
        // Done once at construction; Deref never re-validates.
        T::validate_account(info, index)?;
        let header_ptr: *mut T;
        let disc_len = T::DISCRIMINATOR.len();

        #[cfg(not(feature = "pinocchio"))]
        {
            let data = info.try_borrow_data()?;
            // Validate — this also ensures data[disc_len..] is aligned for T (bytemuck check).
            let _: &T =
                T::load(&data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(index))?;
            // SAFETY: The SBF runtime guarantees account data is valid for the entire
            // instruction. The RefCell borrow is dropped after this block, but the
            // underlying pointer remains valid (the runtime does not move account data).
            header_ptr = unsafe { (data.as_ptr() as *mut u8).add(disc_len) as *mut T };
        }

        #[cfg(feature = "pinocchio")]
        {
            let data = info.data();
            // Validate — ensures disc match and sufficient length.
            let _: &T =
                T::load(data).map_err(|_| NaclacError::InvalidAccountDiscriminator.err(index))?;
            // SAFETY: `data_ptr()` is valid for the instruction lifetime (SBF guarantee).
            // We derive from the const pointer but cast to *mut for DerefMut; the SBF
            // memory model does not enforce provenance tracking.
            header_ptr = unsafe { info.view.data_ptr().add(disc_len) as *mut T };
        }

        Ok(Self {
            info: *info,
            header_ptr,
            _phantom: PhantomData,
        })
    }

    /// `#[account(mut)]` non-init accounts go through this name for
    /// symmetry with the Borsh branch and with `init` accounts, but it does
    /// not skip validation: nothing about a `mut` field proves the account
    /// data at that slot was ever produced by this program's own `init` for
    /// this exact type — a caller can put any same-owner account in an
    /// unconstrained `mut` slot, which is exactly what the discriminator
    /// check exists to catch. An earlier version of this function skipped
    /// the check on the (false) assumption that it was "verified at init
    /// time"; kept as a thin alias, not a fast path, so the name stays
    /// meaningful at call sites without reintroducing that gap.
    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        Self::try_from(info, index)
    }
}

// ---------------------------------------------------------------------------
// Deref / DerefMut — the hot path.
//
// These are single pointer dereferences. No size check, no discriminator check,
// no bytemuck call. The validation was done once in `try_from`.
// Mirrors Anchor's Slab::deref/deref_mut exactly.
// ---------------------------------------------------------------------------

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Deref for Account<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: header_ptr was validated at construction time (try_from) and the
        // SBF runtime guarantees account data does not move during instruction execution.
        unsafe { &*self.header_ptr }
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> DerefMut for Account<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: same as Deref. Mutability (is_writable check) is enforced by
        // `#[account(mut)]` in the generated load_and_validate code before this
        // wrapper is ever constructed.
        unsafe { &mut *self.header_ptr }
    }
}

// ---------------------------------------------------------------------------
// Standard trait implementations
// ---------------------------------------------------------------------------

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> ToAccountInfo for Account<T> {
    #[inline(always)]
    fn to_account_info(&self) -> AccountInfo {
        self.info
    }
}

#[cfg(all(feature = "bytemuck", not(feature = "pinocchio")))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAccountInfos for Account<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info]
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Owner for Account<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> crate::wrappers::ToAddress for Account<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> Account<T> {
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
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(feature = "bytemuck")]
impl<T: NaclacZeroCopy> crate::wrappers::AsRefByteSlice for Account<T> {
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
impl<T: NaclacZeroCopy> crate::wrappers::NaclacAccount for Account<T> {
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
