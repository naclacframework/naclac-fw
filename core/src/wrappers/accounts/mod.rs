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

use crate::prelude::NaclacError;
#[cfg(not(all(feature = "borsh", feature = "wincode")))]
pub use account::Account;
#[cfg(not(all(feature = "borsh", feature = "wincode")))]
pub use interface_account::InterfaceAccount;

/// Discards a Borsh serializer error without running its destructor. Callers
/// replace the error with one that fails the transaction, so the leaked
/// allocation is never observed, and the destructor code stays out of the
/// program binary.
#[cfg(all(
    feature = "borsh",
    not(feature = "pinocchio"),
    not(feature = "wincode")
))]
#[inline(always)]
pub(crate) fn discard_error<E>(error: E) {
    core::mem::forget(error);
}

/// Trait for types that can be zero-copy loaded from raw bytes.
///
/// The data slice passed to `load` must include the discriminator prefix
/// (`Self::DISCRIMINATOR`, 1 to 8 bytes, or empty for foreign types).
/// Validation checks both the discriminator and size.
///
/// No `load_mut` counterpart — confirmed dead code before this refactor
/// (verified by grepping every call site in the repo): `Account::try_from_mut`
/// is a thin alias for `try_from` (both call `load`), and nothing else calls
/// the trait's `load_mut`. The only other `load_mut`
/// in the repo is a separate, macro-generated *inherent* method
/// (`macros/src/component.rs`) that duplicates this logic for
/// off-chain use — it doesn't call this trait method either.
pub trait NaclacZeroCopy:
    bytemuck::Pod + bytemuck::Zeroable + crate::wrappers::Discriminator
{
    /// Fails compilation when the account body would not be correctly
    /// aligned for `Self`: the body starts at offset `DISCRIMINATOR.len()`
    /// in a buffer that is 8-byte aligned, so `Self` must not need more than
    /// 8-byte alignment and the discriminator width must be a multiple of
    /// `Self`'s alignment. Every code path that casts account bytes to `Self`
    /// must call this first.
    #[inline(always)]
    fn assert_layout() {
        const {
            assert!(
                core::mem::align_of::<Self>() <= 8,
                "zero-copy account alignment exceeds the 8-byte alignment of Solana account data",
            );
            assert!(
                Self::DISCRIMINATOR.len() % core::mem::align_of::<Self>() == 0,
                "discriminator width is not a multiple of this zero-copy account's alignment; use a discriminator whose byte length is a multiple of the struct's alignment",
            );
        }
    }

    #[inline(always)]
    fn load(data: &[u8]) -> crate::prelude::Result<&Self> {
        Self::assert_layout();
        let disc = Self::DISCRIMINATOR;
        let disc_len = disc.len();
        let space = core::mem::size_of::<Self>() + disc_len;
        if data.len() < space {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if &data[..disc_len] != disc {
            return Err(NaclacError::InvalidAccountDiscriminator.err(0));
        }
        // SAFETY: T: Pod guarantees any bit pattern is valid. Solana aligns
        // account data buffers to 8 bytes and `assert_layout` proves
        // `disc_len` is a multiple of `align_of::<Self>()`, so the body is
        // aligned. The size check above guarantees `size_of::<Self>()` bytes
        // are available at offset `disc_len`.
        Ok(unsafe { &*(data.as_ptr().add(disc_len) as *const Self) })
    }
}
