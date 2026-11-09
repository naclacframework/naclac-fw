//! The largest number of bytes a value of a type can serialize to under Borsh
//! or Wincode, and whether a value is within the length limits it was declared
//! with.
//!
//! `#[account]` adds the maximum sizes of its fields to get the account's
//! `SPACE`, and checks `within_bounds` before writing the account back.
//! The maximum size is resolved by the compiler through this trait, never
//! guessed from a type's name, so a type alias gives the same answer as the
//! type it names and a type with no known maximum is a compile error.
//!
//! `Vec` and `String` deliberately have no implementation: their size is a
//! property of the field (`#[max_len(N)]`), not of the type. A `#[defined_type]`
//! implements the trait from its own fields.

use crate::prelude::Address;

#[diagnostic::on_unimplemented(
    message = "`{Self}` has no known maximum serialized size",
    label = "no maximum size for this type",
    note = "a `Vec` or `String` field needs `#[max_len(N)]` on the field, and a custom type \
            needs `#[defined_type]`"
)]
pub trait SerializedBounds {
    /// The most bytes a value of this type serializes to.
    const MAX_SIZE: usize;

    /// False when a `Vec` or `String` inside the value is longer than the
    /// `#[max_len]` it was declared with.
    fn within_bounds(&self) -> bool;
}

macro_rules! fixed_size {
    ($($ty:ty => $size:expr),* $(,)?) => {
        $(
            impl SerializedBounds for $ty {
                const MAX_SIZE: usize = $size;

                #[inline(always)]
                fn within_bounds(&self) -> bool {
                    true
                }
            }
        )*
    };
}

fixed_size!(
    u8 => 1, i8 => 1, bool => 1,
    u16 => 2, i16 => 2,
    u32 => 4, i32 => 4, f32 => 4,
    u64 => 8, i64 => 8, f64 => 8,
    u128 => 16, i128 => 16,
);

impl SerializedBounds for Address {
    const MAX_SIZE: usize = 32;

    #[inline(always)]
    fn within_bounds(&self) -> bool {
        true
    }
}

impl<T: SerializedBounds, const N: usize> SerializedBounds for [T; N] {
    const MAX_SIZE: usize = N * T::MAX_SIZE;

    #[inline(always)]
    fn within_bounds(&self) -> bool {
        self.iter().all(SerializedBounds::within_bounds)
    }
}

impl<T: SerializedBounds> SerializedBounds for Option<T> {
    const MAX_SIZE: usize = 1 + T::MAX_SIZE;

    #[inline(always)]
    fn within_bounds(&self) -> bool {
        match self {
            Some(value) => value.within_bounds(),
            None => true,
        }
    }
}

/// The bound of a type whose outermost `Vec`/`String` level takes the
/// `#[max_len]` limit `A`, for a field type `naclac-syn`'s bounds generator
/// cannot see into, such as an alias of `String`. Sizes and checks must equal
/// the ones that generator writes out for a literal `String`, `Vec` and
/// `Option`.
#[diagnostic::on_unimplemented(
    message = "Naclac Error: this `#[max_len]` has more limits than the field has `Vec` or \
               `String` levels to apply them to (`{Self}`)",
    label = "this type takes fewer `#[max_len]` limits"
)]
pub trait MaxLen1<const A: usize> {
    /// The most bytes a value of this type serializes to.
    const MAX_SIZE: usize;

    /// False when a `Vec` or `String` inside the value exceeds its limit.
    fn within_limits(&self) -> bool;
}

/// [`MaxLen1`] for two levels: `A` for the outer `Vec`, `B` for its elements.
#[diagnostic::on_unimplemented(
    message = "Naclac Error: this `#[max_len]` has more limits than the field has `Vec` or \
               `String` levels to apply them to (`{Self}`)",
    label = "this type takes fewer `#[max_len]` limits"
)]
pub trait MaxLen2<const A: usize, const B: usize> {
    /// The most bytes a value of this type serializes to.
    const MAX_SIZE: usize;

    /// False when a `Vec` or `String` inside the value exceeds its limit.
    fn within_limits(&self) -> bool;
}

/// [`MaxLen1`] for three levels, outermost first.
#[diagnostic::on_unimplemented(
    message = "Naclac Error: this `#[max_len]` has more limits than the field has `Vec` or \
               `String` levels to apply them to (`{Self}`)",
    label = "this type takes fewer `#[max_len]` limits"
)]
pub trait MaxLen3<const A: usize, const B: usize, const C: usize> {
    /// The most bytes a value of this type serializes to.
    const MAX_SIZE: usize;

    /// False when a `Vec` or `String` inside the value exceeds its limit.
    fn within_limits(&self) -> bool;
}

impl<const A: usize> MaxLen1<A> for alloc::string::String {
    const MAX_SIZE: usize = 4 + A;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        self.len() <= A
    }
}

impl<T: SerializedBounds, const A: usize> MaxLen1<A> for alloc::vec::Vec<T> {
    const MAX_SIZE: usize = 4 + A * T::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        self.len() <= A && self.iter().all(SerializedBounds::within_bounds)
    }
}

impl<T: MaxLen1<A>, const A: usize> MaxLen1<A> for Option<T> {
    const MAX_SIZE: usize = 1 + T::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        match self {
            Some(value) => value.within_limits(),
            None => true,
        }
    }
}

impl<T: MaxLen1<B>, const A: usize, const B: usize> MaxLen2<A, B> for alloc::vec::Vec<T> {
    const MAX_SIZE: usize = 4 + A * <T as MaxLen1<B>>::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        self.len() <= A && self.iter().all(MaxLen1::within_limits)
    }
}

impl<T: MaxLen2<A, B>, const A: usize, const B: usize> MaxLen2<A, B> for Option<T> {
    const MAX_SIZE: usize = 1 + T::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        match self {
            Some(value) => value.within_limits(),
            None => true,
        }
    }
}

impl<T: MaxLen2<B, C>, const A: usize, const B: usize, const C: usize> MaxLen3<A, B, C>
    for alloc::vec::Vec<T>
{
    const MAX_SIZE: usize = 4 + A * <T as MaxLen2<B, C>>::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        self.len() <= A && self.iter().all(MaxLen2::within_limits)
    }
}

impl<T: MaxLen3<A, B, C>, const A: usize, const B: usize, const C: usize> MaxLen3<A, B, C>
    for Option<T>
{
    const MAX_SIZE: usize = 1 + T::MAX_SIZE;

    #[inline(always)]
    fn within_limits(&self) -> bool {
        match self {
            Some(value) => value.within_limits(),
            None => true,
        }
    }
}

#[cfg(test)]
mod max_len_tests {
    use {
        super::{MaxLen1, MaxLen2, MaxLen3},
        alloc::{string::String, vec, vec::Vec},
    };

    type Label = String;

    #[test]
    fn sizes_match_the_literal_shapes() {
        assert!(<String as MaxLen1<8>>::MAX_SIZE == 4 + 8);
        assert!(<Label as MaxLen1<8>>::MAX_SIZE == 4 + 8);
        assert!(<Vec<u16> as MaxLen1<3>>::MAX_SIZE == 4 + 3 * 2);
        assert!(<Option<String> as MaxLen1<3>>::MAX_SIZE == 1 + 4 + 3);
        assert!(<Vec<String> as MaxLen2<2, 5>>::MAX_SIZE == 4 + 2 * (4 + 5));
        assert!(<Vec<Vec<String>> as MaxLen3<2, 3, 4>>::MAX_SIZE == 4 + 2 * (4 + 3 * (4 + 4)));
    }

    #[test]
    fn limits_are_checked_at_every_level() {
        assert!(<Label as MaxLen1<3>>::within_limits(&String::from("abc")));
        assert!(!<Label as MaxLen1<3>>::within_limits(&String::from("abcd")));
        let names: Vec<String> = vec![String::from("ab"), String::from("abc")];
        assert!(<Vec<String> as MaxLen2<2, 3>>::within_limits(&names));
        assert!(!<Vec<String> as MaxLen2<1, 3>>::within_limits(&names));
        assert!(!<Vec<String> as MaxLen2<2, 2>>::within_limits(&names));
        assert!(<Option<String> as MaxLen1<1>>::within_limits(&None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_are_their_serialized_size() {
        assert_eq!(<u8 as SerializedBounds>::MAX_SIZE, 1);
        assert_eq!(<bool as SerializedBounds>::MAX_SIZE, 1);
        assert_eq!(<u16 as SerializedBounds>::MAX_SIZE, 2);
        assert_eq!(<f32 as SerializedBounds>::MAX_SIZE, 4);
        assert_eq!(<i64 as SerializedBounds>::MAX_SIZE, 8);
        assert_eq!(<u128 as SerializedBounds>::MAX_SIZE, 16);
        assert_eq!(<Address as SerializedBounds>::MAX_SIZE, 32);
    }

    #[test]
    fn an_option_adds_its_tag_byte() {
        assert_eq!(<Option<u64> as SerializedBounds>::MAX_SIZE, 9);
        assert_eq!(<Option<Option<u8>> as SerializedBounds>::MAX_SIZE, 3);
    }

    #[test]
    fn an_array_is_its_length_times_the_element() {
        assert_eq!(<[u16; 5] as SerializedBounds>::MAX_SIZE, 10);
        assert_eq!(<[Option<u8>; 3] as SerializedBounds>::MAX_SIZE, 6);
        assert_eq!(<[[u8; 4]; 2] as SerializedBounds>::MAX_SIZE, 8);
    }

    #[test]
    fn a_type_alias_has_the_size_of_the_type_it_names() {
        type Amount = u64;
        assert_eq!(<Amount as SerializedBounds>::MAX_SIZE, 8);
    }

    #[test]
    fn plain_values_are_always_within_bounds() {
        assert!(7u64.within_bounds());
        assert!(Some([1u8, 2, 3]).within_bounds());
        assert!(None::<u32>.within_bounds());
    }
}
