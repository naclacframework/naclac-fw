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
//! `naclac-core/docs/accounts-consolidation-plan.md`'s truth table), so
//! exactly one `Account<T>` definition is ever compiled.

// ===========================================================================
// Borsh branch — owns a deserialized copy of T
// ===========================================================================

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use crate::prelude::{AccountInfo, Address, BorshDeserialize, BorshSerialize, NaclacError, Result};
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use core::ops::{Deref, DerefMut};

/// Wrapper for Borsh-serialized accounts (The "Borsh Aspect").
///
/// This struct is used in standard Solana programs to handle automatic
/// deserialization and serialization of account data using Borsh.
///
/// naclac uses an 8-byte discriminator at the start of the account data
/// for compatibility with Anchor and other standard tools.
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
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

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Account<T> {
    /// Deserializes the account data into the inner type, validating the 8-byte
    /// discriminator before skipping it. Without this check, two different
    /// `#[component]` types owned by the same program are indistinguishable to
    /// this wrapper — a same-program account of the wrong type would silently
    /// be reinterpreted as `T` via raw Borsh deserialization.
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        T::validate_account(info, index)?;
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

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> crate::wrappers::Owner
    for Account<T>
{
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref for Account<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut
    for Account<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo
    for Account<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::ToAccountInfos for Account<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAddress
    for Account<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
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

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::AsRefByteSlice for Account<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
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

// ===========================================================================
// Zero-copy branch — caches a raw pointer into the account's own buffer
// ===========================================================================
//
// At construction time (`try_from`), we validate the discriminator and size
// ONCE, then cache a raw `*mut T` pointer directly into the account data at
// offset `disc_len`. `Deref`/`DerefMut` then cost exactly one unsafe pointer
// dereference — no size check, no discriminator check, no bytemuck on every
// access. Mirrors Anchor's `Slab<H, HeaderOnly>` design.

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::prelude::{AccountInfo, Address, NaclacError};
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::wrappers::accounts::NaclacZeroCopy;
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::wrappers::{Owner, ToAccountInfo};
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use core::marker::PhantomData;
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use core::ops::{Deref, DerefMut};

/// Wrapper for Zero-Copy accounts.
///
/// Stores a cached raw pointer to the typed data `T` inside the account's data buffer
/// (at offset +8, past the discriminator). `Deref`/`DerefMut` cost a single pointer
/// dereference — no per-access validation overhead.
///
/// Mirrors Anchor's `Slab<H, HeaderOnly>` design for binary-size efficiency.
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
unsafe impl<T: NaclacZeroCopy + Send> Send for Account<T> {}
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
unsafe impl<T: NaclacZeroCopy + Sync> Sync for Account<T> {}

// Manual Clone: raw pointers are Copy so cloning the struct is fine.
#[cfg(feature = "pinocchio")]
impl<T: NaclacZeroCopy> Clone for Account<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(all(not(feature = "pinocchio"), not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Clone for Account<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            info: self.info.clone(),
            header_ptr: self.header_ptr,
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: NaclacZeroCopy> Copy for Account<T> {}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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
        let disc_len = T::discriminator_len();

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
            info: {
                #[cfg(feature = "pinocchio")]
                {
                    *info
                }
                #[cfg(not(feature = "pinocchio"))]
                {
                    info.clone()
                }
            },
            header_ptr,
            _phantom: PhantomData,
        })
    }

    /// Fast-path constructor for `#[account(mut)]` non-init accounts.
    ///
    /// Mirrors Anchor's `Slab::build_mutable`: skips the discriminator check
    /// (it was verified at `init` time) and only validates account size.
    /// Saves ~80–100 CUs per mutable account load on the hot path.
    ///
    /// **Do NOT use for `init` accounts** — those still need `try_from` which
    /// validates the discriminator before handing out a mutable pointer.
    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        T::validate_account(info, index)?;
        let disc_len = T::discriminator_len();
        let space = core::mem::size_of::<T>() + disc_len;

        #[cfg(feature = "pinocchio")]
        {
            // SAFETY: data_ptr() is valid for the instruction lifetime.
            // Size is checked; discriminator verified at init time.
            let data_len = info.view.data_len();
            if data_len < space {
                return Err(NaclacError::AccountDataTooSmall.err(index));
            }
            let header_ptr = unsafe { info.view.data_ptr().add(disc_len) as *mut T };
            Ok(Self {
                info: *info,
                header_ptr,
                _phantom: PhantomData,
            })
        }

        #[cfg(not(feature = "pinocchio"))]
        {
            let data = info.try_borrow_data()?;
            if data.len() < space {
                return Err(NaclacError::AccountDataTooSmall.err(index));
            }
            let header_ptr = unsafe { (data.as_ptr() as *mut u8).add(disc_len) as *mut T };
            Ok(Self {
                info: info.clone(),
                header_ptr,
                _phantom: PhantomData,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Deref / DerefMut — the hot path.
//
// These are single pointer dereferences. No size check, no discriminator check,
// no bytemuck call. The validation was done once in `try_from`.
// Mirrors Anchor's Slab::deref/deref_mut exactly.
// ---------------------------------------------------------------------------

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Deref for Account<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: header_ptr was validated at construction time (try_from) and the
        // SBF runtime guarantees account data does not move during instruction execution.
        unsafe { &*self.header_ptr }
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> ToAccountInfo for Account<T> {
    #[inline(always)]
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

#[cfg(all(not(feature = "pinocchio"), not(feature = "borsh")))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAccountInfos for Account<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Owner for Account<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAddress for Account<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandle<'a> for Account<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for Account<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> crate::wrappers::NaclacAccount for Account<T> {
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> crate::prelude::Result<Self> {
        Self::try_from(info, index)
    }

    #[inline(always)]
    fn try_from_mut(
        info: &crate::prelude::AccountType,
        index: usize,
    ) -> crate::prelude::Result<Self> {
        Self::try_from_mut(info, index)
    }
}
