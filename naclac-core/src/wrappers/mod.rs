// ===========================================================================
// wrappers/mod.rs — Shared traits and module exports for Naclac wrappers
// ===========================================================================

//! # Naclac Core Wrappers
//!
//! Provides zero-cost abstraction wrappers around standard SBF accounts (`AccountInfo`, `AccountView`).
//! These traits and wrappers are heavily utilized by the macro engine to seamlessly route
//! logic between Solana and Pinocchio backends without exposing backend-specific types to the developer.

pub mod accounts;
pub mod cpi_handle;
pub mod interface;
pub mod program;
pub mod signer;
pub mod span;

pub use accounts::*;
pub use cpi_handle::*;
pub use interface::*;
pub use program::*;
pub use signer::*;
pub use span::*;

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::AccountInfo;
use crate::prelude::Address;
#[cfg(feature = "pinocchio")]
use crate::prelude::{AccountInfo, AccountView, PinocchioAddress};

/// Trait to convert a type into an Address.
pub trait ToAddress {
    fn address(&self) -> Address;
}

#[cfg(feature = "pinocchio")]
pub trait ToAccountView {
    fn to_account_view(&self) -> AccountView;
}

pub trait Owner {
    fn program_owner(&self) -> Address;
}

/// Trait to define the discriminator length for an account.
/// Default is 8 bytes (Anchor/Naclac standard).
pub trait Discriminator {
    const DISCRIMINATOR: [u8; 8] = [0u8; 8];
    fn discriminator_len() -> usize {
        8
    }

    /// Extra per-type validation run once at account-wrapper construction
    /// beyond what the discriminator/size check alone can verify from
    /// the raw data slice — e.g. the account's owner, which lives on
    /// `AccountInfo`, not in the data itself. No-op by default; overridden
    /// by types (like naclac-token's `TokenAccount`/`Mint`) that must always
    /// be owned by a specific external program regardless of what
    /// `#[account(...)]` attributes the app author did or didn't write on
    /// that field.
    #[inline(always)]
    fn validate_account(_info: &AccountInfo, _index: usize) -> crate::prelude::Result<()> {
        Ok(())
    }
}

/// Extra per-type validation `InterfaceAccount<T>` runs on `T`'s own raw
/// byte region (the account's data slice past the discriminator) once the
/// owner check has passed. Unlike `Discriminator::validate_account`,
/// `InterfaceAccount<T>::try_from`/`try_from_mut` never call
/// `T::validate_account` (that hook's owner check is what `InterfaceAccount`
/// exists to relax), so this is the only place a `T` used with
/// `InterfaceAccount<T>` can enforce additional layout invariants — e.g.
/// naclac-token's `TokenAccount`/`Mint` validating canonical `COption` tag
/// bytes and initialized-state, permissively (`>=` minimum length) to allow
/// for Token-2022's appended TLV extension region. No-op by default.
pub trait ValidateInterfaceLayout {
    #[inline(always)]
    fn validate_interface_layout(
        _data: &[u8],
    ) -> core::result::Result<(), crate::prelude::NaclacError> {
        Ok(())
    }
}

/// Helper trait for PDA seeds to handle different types safely.
pub trait AsRefByteSlice {
    fn as_ref_byte_slice(&self) -> &[u8];
}

/// Backend-specific trait to convert a wrapper to a raw AccountInfo.
pub trait ToAccountInfo {
    fn to_account_info(&self) -> AccountInfo;
}

/// Trait to convert a type into a list of AccountInfos.
pub trait ToAccountInfos {
    #[cfg(not(feature = "pinocchio"))]
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo>;
}

/// Core trait for Naclac-validated account wrapper types.
/// Defines unified hydration and lifecycle methods for all execution backends.
pub trait NaclacAccount: Sized {
    /// Whether this account type is a signer by default.
    const IS_SIGNER: bool = false;

    /// Whether this account type is writable by default.
    const IS_WRITABLE: bool = false;

    /// Hydrate and validate the account wrapper from an `AccountType`.
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> crate::prelude::Result<Self>;

    /// Fast-path hydration for mutable non-init accounts.
    /// By default, falls back to standard `try_from`.
    #[inline(always)]
    fn try_from_mut(
        info: &crate::prelude::AccountType,
        index: usize,
    ) -> crate::prelude::Result<Self> {
        Self::try_from(info, index)
    }

    /// Executed during instruction teardown to persist changes or perform cleanup.
    #[inline(always)]
    fn exit(&self, _program_id: &crate::prelude::Address) -> crate::prelude::Result<()> {
        Ok(())
    }
}

/// Declares the on-chain address for a program marker type.
pub trait Id {
    fn id() -> Address;
}

/// Declares multiple valid on-chain addresses for an interface program marker.
pub trait Ids {
    fn ids() -> &'static [Address];
}

impl NaclacAccount for AccountInfo {
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, _index: usize) -> crate::prelude::Result<Self> {
        #[cfg(feature = "pinocchio")]
        {
            Ok(*info)
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            Ok(info.clone())
        }
    }
}

#[cfg(feature = "pinocchio")]
impl NaclacAccount for AccountView {
    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, _index: usize) -> crate::prelude::Result<Self> {
        Ok(info.view)
    }
}

#[cfg(all(feature = "no-std", not(feature = "pinocchio")))]
use alloc::boxed::Box;
#[cfg(all(not(feature = "no-std"), not(feature = "pinocchio")))]
use std::boxed::Box;

#[cfg(not(feature = "pinocchio"))]
impl<T: NaclacAccount> NaclacAccount for Box<T> {
    const IS_SIGNER: bool = T::IS_SIGNER;
    const IS_WRITABLE: bool = T::IS_WRITABLE;

    #[inline(always)]
    fn try_from(info: &crate::prelude::AccountType, index: usize) -> crate::prelude::Result<Self> {
        Ok(Box::new(T::try_from(info, index)?))
    }

    #[inline(always)]
    fn try_from_mut(
        info: &crate::prelude::AccountType,
        index: usize,
    ) -> crate::prelude::Result<Self> {
        Ok(Box::new(T::try_from_mut(info, index)?))
    }

    #[inline(always)]
    fn exit(&self, program_id: &crate::prelude::Address) -> crate::prelude::Result<()> {
        (**self).exit(program_id)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: Owner> Owner for Box<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        (**self).program_owner()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: ToAddress> ToAddress for Box<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        (**self).address()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: AsRefByteSlice> AsRefByteSlice for Box<T> {
    #[inline(always)]
    fn as_ref_byte_slice(&self) -> &[u8] {
        (**self).as_ref_byte_slice()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: ToAccountInfo> ToAccountInfo for Box<T> {
    #[inline(always)]
    fn to_account_info(&self) -> AccountInfo {
        (**self).to_account_info()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: ToAccountInfos> ToAccountInfos for Box<T> {
    #[inline(always)]
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        (**self).to_account_infos()
    }
}

// Implementations for primitive types
impl ToAddress for Address {
    fn address(&self) -> Address {
        *self
    }
}

impl ToAddress for &[u8] {
    fn address(&self) -> Address {
        Address::new_from_array((*self).try_into().unwrap_or([0u8; 32]))
    }
}

impl<const N: usize> ToAddress for [u8; N] {
    #[inline(always)]
    fn address(&self) -> Address {
        let mut arr = [0u8; 32];
        let len = N.min(32);
        // SAFETY: len <= min(N, 32) <= 32, so both src and dst are in-bounds.
        // Avoids emitting a bounds-check panic string + digit table.
        unsafe {
            core::ptr::copy_nonoverlapping(self.as_ptr(), arr.as_mut_ptr(), len);
        }
        Address::new_from_array(arr)
    }
}

impl<T: ToAddress + ?Sized> ToAddress for &T {
    fn address(&self) -> Address {
        T::address(*self)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToAccountInfo for AccountInfo {
    fn to_account_info(&self) -> AccountInfo {
        self.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToAccountInfos for AccountInfo {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl ToAddress for AccountInfo {
    fn address(&self) -> Address {
        *self.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl Owner for AccountInfo {
    fn program_owner(&self) -> Address {
        *self.owner
    }
}

#[cfg(feature = "pinocchio")]
impl ToAccountInfo for AccountView {
    fn to_account_info(&self) -> AccountInfo {
        AccountInfo { view: *self }
    }
}

#[cfg(feature = "pinocchio")]
impl ToAddress for AccountView {
    fn address(&self) -> Address {
        Address::from_address(self.address())
    }
}

#[cfg(feature = "pinocchio")]
impl ToAddress for PinocchioAddress {
    fn address(&self) -> Address {
        Address::from_address(self)
    }
}

#[cfg(feature = "pinocchio")]
impl Owner for AccountView {
    fn program_owner(&self) -> Address {
        Address::from_address(self.owner())
    }
}

#[cfg(feature = "pinocchio")]
impl ToAccountView for AccountInfo {
    fn to_account_view(&self) -> AccountView {
        self.view
    }
}

#[cfg(feature = "pinocchio")]
impl ToAddress for AccountInfo {
    fn address(&self) -> Address {
        ToAddress::address(&self.view)
    }
}

#[cfg(feature = "pinocchio")]
impl Owner for AccountInfo {
    fn program_owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }
}

#[cfg(feature = "pinocchio")]
impl ToAccountInfo for AccountInfo {
    fn to_account_info(&self) -> AccountInfo {
        *self
    }
}

// --- AsRefByteSlice Implementations ---

impl AsRefByteSlice for Address {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_ref()
    }
}

#[cfg(feature = "pinocchio")]
impl AsRefByteSlice for PinocchioAddress {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsRefByteSlice for [u8] {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self
    }
}

impl<const N: usize> AsRefByteSlice for [u8; N] {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsRefByteSlice for &str {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[cfg(feature = "pinocchio")]
impl AsRefByteSlice for crate::wrappers::ZcString {
    #[inline(always)]
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl AsRefByteSlice for crate::prelude::String {
    #[inline(always)]
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_bytes()
    }
}

macro_rules! impl_as_ref_byte_slice {
    ($($t:ty),*) => {
        $(
            impl AsRefByteSlice for $t {
                fn as_ref_byte_slice(&self) -> &[u8] {
                    // SAFETY: We are converting a primitive, trivially copyable scalar/array into a byte slice.
                    // This is perfectly safe as we are taking a pointer to `self` and sizing it to `size_of::<T>()`.
                    unsafe {
                        core::slice::from_raw_parts(
                            self as *const $t as *const u8,
                            core::mem::size_of::<$t>(),
                        )
                    }
                }
            }
        )*
    };
}

impl_as_ref_byte_slice!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl<T: AsRefByteSlice + ?Sized> AsRefByteSlice for &T {
    fn as_ref_byte_slice(&self) -> &[u8] {
        T::as_ref_byte_slice(*self)
    }
}

#[cfg(kani)]
mod address_kani_proofs {
    use super::*;

    /// Proves `<[u8; N] as ToAddress>::address` never panics/UB's and copies
    /// exactly `min(N, 32)` bytes, zero-padding the rest — for `N` below,
    /// at, and above 32. `N` is a const generic, so each width needs its own
    /// concrete monomorphization; these three cover every size relationship
    /// `min(N, 32)` can have.
    macro_rules! prove_address_for_width {
        ($proof_name:ident, $n:expr) => {
            #[kani::proof]
            fn $proof_name() {
                let arr: [u8; $n] = kani::any();
                let addr = arr.address();
                let bytes = addr.as_ref_byte_slice();
                let len = ($n as usize).min(32);
                assert_eq!(bytes.len(), 32);
                assert_eq!(&bytes[..len], &arr[..len]);
                assert!(bytes[len..].iter().all(|b| *b == 0));
            }
        };
    }

    prove_address_for_width!(prove_address_width_below_32, 16);
    prove_address_for_width!(prove_address_width_exactly_32, 32);
    prove_address_for_width!(prove_address_width_above_32, 64);
}
