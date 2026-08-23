// ===========================================================================
// wrappers/accounts/interface_account.rs — InterfaceAccount<T>
// ===========================================================================

//! `InterfaceAccount<T>` accepts an account owned by either the legacy Token
//! program or Token-2022, regardless of what `T`'s own
//! `Discriminator::validate_account` says (e.g. `TokenAccount`/`Mint`'s own
//! `validate_account` is strict — Token-program-only). This can't be done
//! via a type alias to `Account<T>` (an alias carries no behavior of its own
//! to override `T`'s existing `validate_account` with — `InterfaceAccount<Mint>`
//! would just *be* `Account<Mint>`, identically strict). So this is a
//! genuinely separate wrapper, structurally mirroring `Account<T>`
//! (`accounts/account.rs`) almost exactly — same cfg-routed Borsh-vs-zero-copy
//! split, same set of trait impls — except `try_from`/`try_from_mut` check
//! the owner directly instead of calling `T::validate_account`.
//!
//! Hardcoding `TOKEN_PROGRAM_ID`/`TOKEN_2022_PROGRAM_ID` here (rather than
//! taking the allowed-owner set generically) matches existing naclac-core
//! precedent: `wrappers/interface.rs`'s `TokenInterface` marker already
//! hardcodes the same two IDs for `Program<TokenInterface>`/
//! `Interface<TokenInterface>` fields. This type is specifically the
//! Token/Token-2022 dual-owner wrapper, not a fully generic multi-owner
//! mechanism — a different future multi-owner need would be its own file in
//! this same `accounts/` folder, not a generalization of this one.

// ===========================================================================
// Borsh branch — owns a deserialized copy of T
// ===========================================================================

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use crate::prelude::{
    AccountInfo, Address, BorshDeserialize, BorshSerialize, NaclacError, Owner, Result,
    TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use crate::wrappers::{ToAccountInfo, ToAddress};
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
use core::ops::{Deref, DerefMut};

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
fn check_interface_owner(
    info: &AccountInfo,
    index: usize,
) -> core::result::Result<(), NaclacError> {
    let owner = Owner::program_owner(info);
    if owner != TOKEN_PROGRAM_ID && owner != TOKEN_2022_PROGRAM_ID {
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
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
#[derive(Clone)]
pub struct InterfaceAccount<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> {
    pub info: AccountInfo,
    pub data: T,
    // Deliberately private — see `accounts/account.rs`'s `Account<T>` doc
    // comment on its own `index` field for why.
    index: usize,
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<
        T: BorshDeserialize
            + BorshSerialize
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > InterfaceAccount<T>
{
    pub fn try_from(info: &AccountInfo, index: usize) -> Result<Self> {
        check_interface_owner(info, index).map_err(|e| e.err(index))?;
        let raw = info.try_borrow_data()?;
        let disc_len = T::discriminator_len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(index));
        }
        if raw[..disc_len] != T::DISCRIMINATOR[..disc_len] {
            return Err(NaclacError::AccountNotInitialized.err(index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(index))?;

        let mut reader = &raw[disc_len..];
        let data = T::deserialize(&mut reader)
            .map_err(|_| NaclacError::DeserializationFailed.err(index))?;

        Ok(Self {
            info: info.clone(),
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
        check_interface_owner(&self.info, self.index).map_err(|e| e.err(self.index))?;
        let raw = self.info.try_borrow_data()?;
        let disc_len = T::discriminator_len();
        if raw.len() < disc_len {
            return Err(NaclacError::AccountDataTooSmall.err(self.index));
        }
        if raw[..disc_len] != T::DISCRIMINATOR[..disc_len] {
            return Err(NaclacError::AccountNotInitialized.err(self.index));
        }
        T::validate_interface_layout(&raw[disc_len..]).map_err(|e| e.err(self.index))?;

        let mut reader = &raw[disc_len..];
        self.data = T::deserialize(&mut reader)
            .map_err(|_| NaclacError::DeserializationFailed.err(self.index))?;
        Ok(())
    }

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
    for InterfaceAccount<T>
{
    fn program_owner(&self) -> Address {
        *self.info.owner
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> Deref
    for InterfaceAccount<T>
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> DerefMut
    for InterfaceAccount<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAccountInfo
    for InterfaceAccount<T>
{
    fn to_account_info(&self) -> AccountInfo {
        self.info.clone()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::ToAccountInfos for InterfaceAccount<T>
{
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator> ToAddress
    for InterfaceAccount<T>
{
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
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

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandle<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<'a, T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: BorshDeserialize + BorshSerialize + crate::wrappers::Discriminator>
    crate::wrappers::AsRefByteSlice for InterfaceAccount<T>
{
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.info.key.as_ref()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<
        T: BorshDeserialize
            + BorshSerialize
            + crate::wrappers::Discriminator
            + crate::wrappers::ValidateInterfaceLayout,
    > crate::wrappers::NaclacAccount for InterfaceAccount<T>
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::prelude::{
    AccountInfo, Address, NaclacError, Owner, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::wrappers::accounts::NaclacZeroCopy;
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use crate::wrappers::ToAccountInfo;
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use core::marker::PhantomData;
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
use core::ops::{Deref, DerefMut};

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
fn check_interface_owner_zc(
    info: &AccountInfo,
    index: usize,
) -> core::result::Result<(), NaclacError> {
    let owner = Owner::program_owner(info);
    if owner != TOKEN_PROGRAM_ID && owner != TOKEN_2022_PROGRAM_ID {
        return Err(NaclacError::ConstraintOwner);
    }
    let _ = index;
    Ok(())
}

/// Zero-copy-backed `InterfaceAccount<T>` — see module doc. Field shape and
/// every method here mirror `Account<T>`'s zero-copy branch
/// (`accounts/account.rs`) exactly, except `try_from`/`try_from_mut` check
/// the owner directly instead of calling `T::validate_account`.
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
pub struct InterfaceAccount<T: NaclacZeroCopy> {
    pub info: AccountInfo,
    header_ptr: *mut T,
    pub _phantom: PhantomData<T>,
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
unsafe impl<T: NaclacZeroCopy + Send> Send for InterfaceAccount<T> {}
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
unsafe impl<T: NaclacZeroCopy + Sync> Sync for InterfaceAccount<T> {}

#[cfg(feature = "pinocchio")]
impl<T: NaclacZeroCopy> Clone for InterfaceAccount<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(all(not(feature = "pinocchio"), not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Clone for InterfaceAccount<T> {
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
impl<T: NaclacZeroCopy> Copy for InterfaceAccount<T> {}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy + crate::wrappers::ValidateInterfaceLayout> InterfaceAccount<T> {
    pub fn try_from(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        check_interface_owner_zc(info, index).map_err(|e| e.err(index))?;
        let header_ptr: *mut T;
        let disc_len = T::discriminator_len();

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

    #[inline(always)]
    pub fn try_from_mut(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        check_interface_owner_zc(info, index).map_err(|e| e.err(index))?;
        let disc_len = T::discriminator_len();
        let space = core::mem::size_of::<T>() + disc_len;

        #[cfg(feature = "pinocchio")]
        {
            let data = info.data();
            if data.len() < space {
                return Err(NaclacError::AccountDataTooSmall.err(index));
            }
            T::validate_interface_layout(&data[disc_len..]).map_err(|e| e.err(index))?;
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
            T::validate_interface_layout(&data[disc_len..]).map_err(|e| e.err(index))?;
            let header_ptr = unsafe { (data.as_ptr() as *mut u8).add(disc_len) as *mut T };
            Ok(Self {
                info: info.clone(),
                header_ptr,
                _phantom: PhantomData,
            })
        }
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
        check_interface_owner_zc(&self.info, 0).map_err(|e| e.into())
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Deref for InterfaceAccount<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: header_ptr was validated at construction time (try_from) and the
        // SBF runtime guarantees account data does not move during instruction execution.
        unsafe { &*self.header_ptr }
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> DerefMut for InterfaceAccount<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: same as Deref.
        unsafe { &mut *self.header_ptr }
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> ToAccountInfo for InterfaceAccount<T> {
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
impl<T: NaclacZeroCopy> crate::wrappers::ToAccountInfos for InterfaceAccount<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> Owner for InterfaceAccount<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAddress for InterfaceAccount<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandle<'a> for InterfaceAccount<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandleMut<'a>
    for InterfaceAccount<T>
{
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
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

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: NaclacZeroCopy + crate::wrappers::ValidateInterfaceLayout> crate::wrappers::NaclacAccount
    for InterfaceAccount<T>
{
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
