//! `PodValidate`: the check that stored bytes hold a valid value of a type.
//!
//! The plain-data types here accept every byte pattern as a value of the Rust
//! type, so a malformed stored length or tag cannot be rejected by the cast.
//! `PodValidate` is the rejection: it runs once, on the raw bytes, where bytes
//! enter a program (an account load, an instruction argument, a stored enum
//! variant) and before they are cast to the type. Inside a handler the
//! accessors can then rely on valid contents.
//!
//! It reads only the bytes it is given, so it is sound at any alignment and
//! never forms a reference to a packed field.

use {crate::error::PodError, core::mem::size_of};

/// A type whose stored bytes can be checked for validity.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be validated as a stored field",
    label = "no `PodValidate` impl",
    note = "a `#[defined_type]` enum field must be written `ZcEnum<EnumName>`",
    note = "otherwise use a plain integer, `Address`, `Bool`, a naclac-pod type or a `#[defined_type]` struct"
)]
pub trait PodValidate {
    /// True when `validate_bytes` always succeeds, so callers may skip it.
    const TRIVIAL: bool = false;

    /// Checks that `bytes`, which must be exactly `size_of::<Self>()` long,
    /// hold a valid value of `Self`.
    fn validate_bytes(bytes: &[u8]) -> Result<(), PodError>;
}

macro_rules! trivially_valid {
    ($($ty:ty),* $(,)?) => {
        $(
            impl PodValidate for $ty {
                const TRIVIAL: bool = true;

                #[inline(always)]
                fn validate_bytes(_: &[u8]) -> Result<(), PodError> {
                    Ok(())
                }
            }
        )*
    };
}

trivially_valid!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

#[cfg(feature = "solana")]
trivially_valid!(solana_address::Address);

impl<T: PodValidate, const N: usize> PodValidate for [T; N] {
    const TRIVIAL: bool = T::TRIVIAL;

    #[inline(always)]
    fn validate_bytes(bytes: &[u8]) -> Result<(), PodError> {
        if Self::TRIVIAL {
            return Ok(());
        }
        let size = size_of::<T>();
        for index in 0..N {
            T::validate_bytes(&bytes[index * size..(index + 1) * size])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::Bool};

    #[test]
    fn plain_integers_are_trivially_valid() {
        const { assert!(<u64 as PodValidate>::TRIVIAL) };
        const { assert!(<[u8; 32] as PodValidate>::TRIVIAL) };
        assert!(<u32 as PodValidate>::validate_bytes(&[0xFF; 4]) == Ok(()));
    }

    #[test]
    fn an_array_validates_every_element() {
        const { assert!(!<[Bool; 3] as PodValidate>::TRIVIAL) };
        assert!(<[Bool; 3] as PodValidate>::validate_bytes(&[0, 1, 1]) == Ok(()));
        assert!(
            <[Bool; 3] as PodValidate>::validate_bytes(&[0, 1, 2]) == Err(PodError::InvalidTag)
        );
        assert!(
            <[[Bool; 2]; 2] as PodValidate>::validate_bytes(&[1, 0, 0, 7])
                == Err(PodError::InvalidTag)
        );
    }
}
