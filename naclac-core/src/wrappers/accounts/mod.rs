// ===========================================================================
// wrappers/accounts/mod.rs — shared account-wrapper plumbing
// ===========================================================================

//! naclac's account-wrapper types (`Account<T>`, `InterfaceAccount<T>`),
//! each in its own file here, cfg-routing internally between the Borsh
//! backend and the zero-copy backend (pinocchio, or solana without the
//! `borsh` feature). Any future wrapper type follows the same pattern: its
//! own file in this directory, reusing `NaclacZeroCopy` below as needed.

pub mod account;
pub mod interface_account;

pub use account::Account;
pub use interface_account::InterfaceAccount;

use crate::prelude::NaclacError;

/// Trait for types that can be zero-copy loaded from raw bytes.
///
/// The data slice passed to `load` must include the 8-byte discriminator
/// prefix. Validation checks both the discriminator and size.
///
/// No `load_mut` counterpart — confirmed dead code before this refactor
/// (verified by grepping every call site in the repo): `Account::try_from_mut`
/// deliberately skips straight to computing the cached pointer without
/// calling `load`/`load_mut` at all (the documented hot-path optimization),
/// and nothing else calls the trait's `load_mut`. The only other `load_mut`
/// in the repo is a separate, macro-generated *inherent* method
/// (`naclac-macros/src/component.rs`) that duplicates this logic for
/// off-chain use — it doesn't call this trait method either.
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
}
