//! # Compile-time Borrow-Checked CPI Handles
//!
//! Provides the `CpiHandle` and `CpiHandleMut` wrapper types which enforce
//! Solana account lifetimes and Rust's aliasing rules at compile-time during CPIs.

use crate::prelude::AccountInfo;
use core::marker::PhantomData;

/// An immutable CPI handle representing a shared borrow of an account.
#[repr(transparent)]
pub struct CpiHandle<'a> {
    pub info: AccountInfo,
    pub _phantom: PhantomData<&'a ()>,
}

/// A mutable CPI handle representing an exclusive borrow of an account.
#[repr(transparent)]
pub struct CpiHandleMut<'a> {
    pub info: AccountInfo,
    pub _phantom: PhantomData<&'a mut ()>,
}

impl<'a> From<CpiHandleMut<'a>> for CpiHandle<'a> {
    #[inline(always)]
    fn from(handle: CpiHandleMut<'a>) -> Self {
        Self {
            info: handle.info,
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> Clone for CpiHandle<'a> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a> Clone for CpiHandle<'a> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            info: self.info.clone(),
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> Copy for CpiHandle<'a> {}

/// Trait implemented by wrappers that can be converted into an immutable CPI handle.
pub trait ToCpiHandle<'a> {
    fn to_cpi_handle(&'a self) -> CpiHandle<'a>;
}

/// Trait implemented by wrappers that can be converted into a mutable CPI handle.
pub trait ToCpiHandleMut<'a> {
    fn to_cpi_handle_mut(&'a mut self) -> CpiHandleMut<'a>;
}

#[cfg(feature = "pinocchio")]
impl<'a> ToCpiHandle<'a> for AccountInfo {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> CpiHandle<'a> {
        CpiHandle {
            info: *self,
            _phantom: PhantomData,
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a> ToCpiHandle<'a> for AccountInfo {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> CpiHandle<'a> {
        CpiHandle {
            info: self.clone(),
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> ToCpiHandleMut<'a> for AccountInfo {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> CpiHandleMut<'a> {
        CpiHandleMut {
            info: *self,
            _phantom: PhantomData,
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a> ToCpiHandleMut<'a> for AccountInfo {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> CpiHandleMut<'a> {
        CpiHandleMut {
            info: self.clone(),
            _phantom: PhantomData,
        }
    }
}

use crate::prelude::Address;
use crate::wrappers::ToAddress;

impl<'a> ToAddress for CpiHandle<'a> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

impl<'a> ToAddress for CpiHandleMut<'a> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

#[cfg(all(feature = "no-std", not(feature = "pinocchio")))]
use alloc::boxed::Box;
#[cfg(all(not(feature = "no-std"), not(feature = "pinocchio")))]
use std::boxed::Box;

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: ToCpiHandle<'a>> ToCpiHandle<'a> for Box<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> CpiHandle<'a> {
        (**self).to_cpi_handle()
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<'a, T: ToCpiHandleMut<'a>> ToCpiHandleMut<'a> for Box<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> CpiHandleMut<'a> {
        (**self).to_cpi_handle_mut()
    }
}
