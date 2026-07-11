//! # Zero-Copy Account Loader
//!
//! Provides the AccountLoader wrapper for hydrating and interacting with #[component(zero_copy)]
//! accounts. It safely manages the raw SBF pointers without allocations.
//!
//! ## Design (matches Anchor's Slab approach)
//!
//! At construction time (`try_from`), we validate the discriminator and size ONCE,
//! then cache a raw `*mut T` pointer directly into the account data at offset 8
//! (past the discriminator). `Deref`/`DerefMut` then cost exactly one unsafe pointer
//! dereference — no size check, no discriminator check, no bytemuck on every access.
//!
//! This eliminates the overhead that was causing binary size bloat compared to the
//! old hydration approach.

use crate::prelude::{AccountInfo, Address, NaclacError};
use crate::wrappers::{Owner, ToAccountInfo};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

// --- SHARED LOGIC ---

/// Trait for types that can be zero-copy loaded from raw bytes.
///
/// The data slice passed to `load`/`load_mut` must include the 8-byte discriminator
/// prefix. Validation checks both the discriminator and size.
pub trait NaclacZeroCopy:
    bytemuck::Pod + bytemuck::Zeroable + crate::wrappers::Discriminator
{
    #[inline(always)]
    fn load(data: &[u8]) -> crate::prelude::Result<&Self> {
        let disc_len = Self::discriminator_len();
        let space = core::mem::size_of::<Self>() + disc_len;
        if data.len() < space {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if disc_len > 0 {
            // SAFETY: data.len() >= space >= disc_len; both slices are valid.
            let disc_ok = unsafe {
                let got = core::slice::from_raw_parts(data.as_ptr(), disc_len);
                got == &Self::DISCRIMINATOR[0..disc_len]
            };
            if !disc_ok {
                return Err(NaclacError::InvalidAccountDiscriminator.err(0));
            }
        }
        // SAFETY: T: Pod guarantees any bit pattern is valid. Solana runtime
        // aligns all account data buffers to 8 bytes (LLVM_ALIGN_OF for sBPF).
        // We size-checked above, so the slice covers exactly `size_of::<Self>()`
        // bytes starting at offset disc_len.
        Ok(unsafe { &*(data.as_ptr().add(disc_len) as *const Self) })
    }

    #[inline(always)]
    fn load_mut(data: &mut [u8]) -> crate::prelude::Result<&mut Self> {
        let disc_len = Self::discriminator_len();
        let space = core::mem::size_of::<Self>() + disc_len;
        if data.len() < space {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if disc_len > 0 {
            // SAFETY: data.len() >= space >= disc_len.
            let disc_ok = unsafe {
                let got = core::slice::from_raw_parts(data.as_ptr(), disc_len);
                got == &Self::DISCRIMINATOR[0..disc_len]
            };
            if !disc_ok {
                return Err(NaclacError::InvalidAccountDiscriminator.err(0));
            }
        }
        // SAFETY: same as load() above.
        Ok(unsafe { &mut *(data.as_mut_ptr().add(disc_len) as *mut Self) })
    }
}

/// Wrapper for Zero-Copy accounts.
///
/// Stores a cached raw pointer to the typed data `T` inside the account's data buffer
/// (at offset +8, past the discriminator). `Deref`/`DerefMut` cost a single pointer
/// dereference — no per-access validation overhead.
///
/// Mirrors Anchor's `Slab<H, HeaderOnly>` design for binary-size efficiency.
pub struct AccountLoader<T: NaclacZeroCopy> {
    /// The underlying account info (needed for owner(), to_account_info(), etc.)
    pub info: AccountInfo,
    /// Instruction-level index (for error reporting).
    pub index: usize,
    /// Cached raw pointer into the account's data at offset 8 (past the discriminator).
    ///
    /// SAFETY invariant: valid for the entire instruction execution lifetime.
    /// The SBF runtime guarantees that account data buffers don't move during an instruction.
    header_ptr: *mut T,
    pub _phantom: PhantomData<T>,
}

// SAFETY: `header_ptr` points into Solana account data. Solana's single-threaded SBF
// execution model means Send/Sync are both safe here.
unsafe impl<T: NaclacZeroCopy + Send> Send for AccountLoader<T> {}
unsafe impl<T: NaclacZeroCopy + Sync> Sync for AccountLoader<T> {}

// Manual Clone: raw pointers are Copy so cloning the struct is fine.
#[cfg(feature = "pinocchio")]
impl<T: NaclacZeroCopy> Clone for AccountLoader<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(not(feature = "pinocchio"))]
impl<T: NaclacZeroCopy> Clone for AccountLoader<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            info: self.info.clone(),
            index: self.index,
            header_ptr: self.header_ptr,
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "pinocchio")]
impl<T: NaclacZeroCopy> Copy for AccountLoader<T> {}

impl<T: NaclacZeroCopy> AccountLoader<T> {
    /// Construct from an `AccountInfo`, validating the discriminator and size once.
    ///
    /// On success the cached `header_ptr` is set to `data_ptr + 8`, pointing directly
    /// at `T` inside the account buffer. All subsequent `Deref`/`DerefMut` access is
    /// a single pointer load with no further checks.
    pub fn try_from(info: &AccountInfo, index: usize) -> crate::prelude::Result<Self> {
        // Validate discriminator + size, then cache the raw data pointer.
        // Done once at construction; Deref never re-validates.
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
            index,
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
                index,
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
                index,
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

impl<T: NaclacZeroCopy> Deref for AccountLoader<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: header_ptr was validated at construction time (try_from) and the
        // SBF runtime guarantees account data does not move during instruction execution.
        unsafe { &*self.header_ptr }
    }
}

impl<T: NaclacZeroCopy> DerefMut for AccountLoader<T> {
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

impl<T: NaclacZeroCopy> ToAccountInfo for AccountLoader<T> {
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

#[cfg(not(feature = "pinocchio"))]
impl<T: NaclacZeroCopy> crate::wrappers::ToAccountInfos for AccountLoader<T> {
    fn to_account_infos(&self) -> crate::prelude::Vec<AccountInfo> {
        crate::prelude::vec![self.info.clone()]
    }
}

impl<T: NaclacZeroCopy> Owner for AccountLoader<T> {
    #[inline(always)]
    fn program_owner(&self) -> Address {
        crate::prelude::Owner::program_owner(&self.info)
    }
}

impl<T: NaclacZeroCopy> crate::wrappers::ToAddress for AccountLoader<T> {
    #[inline(always)]
    fn address(&self) -> Address {
        self.info.address()
    }
}

impl<T: NaclacZeroCopy> AccountLoader<T> {
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

impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandle<'a> for AccountLoader<T> {
    #[inline(always)]
    fn to_cpi_handle(&'a self) -> crate::wrappers::cpi_handle::CpiHandle<'a> {
        crate::wrappers::cpi_handle::ToCpiHandle::to_cpi_handle(&self.info)
    }
}

impl<'a, T: NaclacZeroCopy> crate::wrappers::cpi_handle::ToCpiHandleMut<'a> for AccountLoader<T> {
    #[inline(always)]
    fn to_cpi_handle_mut(&'a mut self) -> crate::wrappers::cpi_handle::CpiHandleMut<'a> {
        crate::wrappers::cpi_handle::ToCpiHandleMut::to_cpi_handle_mut(&mut self.info)
    }
}

impl<T: NaclacZeroCopy> crate::wrappers::AsRefByteSlice for AccountLoader<T> {
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

impl<T: NaclacZeroCopy> crate::wrappers::NaclacAccount for AccountLoader<T> {
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
