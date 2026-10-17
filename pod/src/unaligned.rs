//! Marker for plain-data types of alignment 1.
//!
//! A reference to a value is only valid at an address the type's alignment
//! divides. Types of alignment 1 divide every address, so a slice of them can
//! be borrowed straight out of a packed struct. Every other plain-data type is
//! read and written by value.

use {crate::Bool, bytemuck::Pod};

/// A plain-data type whose alignment is 1.
///
/// # Safety
/// `core::mem::align_of::<Self>()` must be 1. [`FixedVec`](crate::FixedVec)
/// borrows `&[Self]` from unaligned memory on the strength of this.
pub unsafe trait Unaligned: Pod {}

macro_rules! unaligned {
    ($($ty:ty),* $(,)?) => {
        $(
            // SAFETY: asserted on the next line.
            unsafe impl Unaligned for $ty {}
            const _: () = assert!(core::mem::align_of::<$ty>() == 1);
        )*
    };
}

unaligned!(u8, i8, Bool);

// SAFETY: an array has its element's alignment.
unsafe impl<T: Unaligned, const N: usize> Unaligned for [T; N] {}

#[cfg(feature = "solana")]
const _: () = assert!(core::mem::align_of::<solana_address::Address>() == 1);

// SAFETY: asserted just above.
#[cfg(feature = "solana")]
unsafe impl Unaligned for solana_address::Address {}
