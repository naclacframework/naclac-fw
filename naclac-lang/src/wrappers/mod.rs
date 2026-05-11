// ===========================================================================
// wrappers/mod.rs — Shared traits and module exports for Naclac wrappers
// ===========================================================================

//! # Naclac Core Wrappers
//!
//! Provides zero-cost abstraction wrappers around standard SBF accounts (`AccountInfo`, `AccountView`).
//! These traits and wrappers are heavily utilized by the macro engine to seamlessly route
//! logic between Solana and Pinocchio backends without exposing backend-specific types to the developer.

#[cfg(any(feature = "pinocchio", all(feature = "solana", feature = "borsh")))]
pub mod account;
pub mod account_loader;
pub mod signer;
pub mod program;
pub mod interface;
pub mod keyed_ref;
pub mod span;

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use account::*;
pub use account_loader::*;
pub use signer::*;
pub use program::*;
pub use interface::*;
pub use keyed_ref::*;
pub use span::*;

use crate::prelude::Pubkey;
#[cfg(not(feature = "pinocchio"))]
use crate::prelude::AccountInfo;
#[cfg(feature = "pinocchio")]
use crate::prelude::{AccountView, Address, AccountInfo};

/// Trait to convert a type into a Pubkey.
pub trait ToPubkey {
    fn to_pubkey(&self) -> Pubkey;
}

/// Trait to access the key of an account wrapper.
pub trait Key {
    fn key(&self) -> Pubkey;
}

#[cfg(feature = "pinocchio")]
pub trait ToAccountView {
    fn to_account_view(&self) -> AccountView;
}

pub trait Owner {
    fn owner(&self) -> Pubkey;
}

/// Trait to define the discriminator length for an account.
/// Default is 8 bytes (Anchor/Naclac standard).
pub trait Discriminator {
    fn discriminator_len() -> usize {
        8
    }
}

/// Helper trait for PDA seeds to handle different types safely.
pub trait AsRefByteSlice {
    fn as_ref_byte_slice(&self) -> &[u8];
}

/// Backend-specific trait to convert a wrapper to a raw AccountInfo.
pub trait ToAccountInfo<'info> {
    #[cfg(not(feature = "pinocchio"))]
    fn to_account_info(&self) -> AccountInfo<'info>;

    #[cfg(feature = "pinocchio")]
    fn to_account_info(&self) -> AccountInfo<'info>;
}

/// Trait to convert a type into a list of AccountInfos.
pub trait ToAccountInfos<'info> {
    #[cfg(not(feature = "pinocchio"))]
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>>;
}

// Implementations for primitive types
impl ToPubkey for Pubkey {
    fn to_pubkey(&self) -> Pubkey {
        *self
    }
}

impl ToPubkey for &[u8] {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::new_from_array((*self).try_into().unwrap_or([0u8; 32]))
    }
}

impl<const N: usize> ToPubkey for [u8; N] {
    fn to_pubkey(&self) -> Pubkey {
        let mut arr = [0u8; 32];
        let len = N.min(32);
        arr[..len].copy_from_slice(&self[..len]);
        Pubkey::new_from_array(arr)
    }
}

impl<'a, T: ToPubkey + ?Sized> ToPubkey for &'a T {
    fn to_pubkey(&self) -> Pubkey {
        T::to_pubkey(*self)
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> ToAccountInfo<'info> for AccountInfo<'info> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.clone()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> ToAccountInfos<'info> for AccountInfo<'info> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo<'info>> {
        crate::prelude::vec![self.clone()]
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> ToPubkey for AccountInfo<'info> {
    fn to_pubkey(&self) -> Pubkey {
        *self.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> Key for AccountInfo<'info> {
    fn key(&self) -> Pubkey {
        *self.key
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'info> Owner for AccountInfo<'info> {
    fn owner(&self) -> Pubkey {
        *self.owner
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToAccountInfo<'info> for AccountView {
    fn to_account_info(&self) -> AccountInfo<'info> {
        AccountInfo {
            view: self.clone(),
            _phantom: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl ToPubkey for AccountView {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::from_address(self.address())
    }
}

#[cfg(feature = "pinocchio")]
impl ToPubkey for Address {
    fn to_pubkey(&self) -> Pubkey {
        Pubkey::from_address(self)
    }
}

#[cfg(feature = "pinocchio")]
impl Key for Address {
    fn key(&self) -> Pubkey {
        Pubkey::from_address(self)
    }
}

#[cfg(feature = "pinocchio")]
impl Key for AccountView {
    fn key(&self) -> Pubkey {
        Pubkey::from_address(self.address())
    }
}

#[cfg(feature = "pinocchio")]
impl Owner for AccountView {
    fn owner(&self) -> Pubkey {
        // SAFETY: Pinocchio's `owner()` directly reads from the raw SBF VM pointer. 
        // We are guaranteed validity because the `AccountView` is initialized directly by the runtime.
        Pubkey::from_address(unsafe { self.owner() })
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToAccountView for AccountInfo<'info> {
    fn to_account_view(&self) -> AccountView {
        self.view.clone()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToPubkey for AccountInfo<'info> {
    fn to_pubkey(&self) -> Pubkey {
        self.view.to_pubkey()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> Key for AccountInfo<'info> {
    fn key(&self) -> Pubkey {
        self.view.key()
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> Owner for AccountInfo<'info> {
    fn owner(&self) -> Pubkey {
        // SAFETY: Pinocchio's `owner()` directly reads from the raw SBF VM pointer.
        // We are guaranteed validity because the `AccountView` is initialized directly by the runtime.
        Pubkey::from_address(unsafe { self.view.owner() })
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> ToAccountInfo<'info> for AccountInfo<'info> {
    fn to_account_info(&self) -> AccountInfo<'info> {
        self.clone()
    }
}

// --- AsRefByteSlice Implementations ---

impl AsRefByteSlice for Pubkey {
    fn as_ref_byte_slice(&self) -> &[u8] {
        self.as_ref()
    }
}

#[cfg(feature = "pinocchio")]
impl AsRefByteSlice for Address {
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
impl AsRefByteSlice for crate::wrappers::ZcString<'_> {
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

impl<'a, T: AsRefByteSlice + ?Sized> AsRefByteSlice for &'a T {
    fn as_ref_byte_slice(&self) -> &[u8] {
        T::as_ref_byte_slice(*self)
    }
}

