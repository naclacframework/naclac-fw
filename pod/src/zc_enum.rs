//! # Zero-Copy Enum Fields
//!
//! `ZcEnum<E>` is how a zero-copy struct stores a `#[defined_type]` enum `E`.
//! An enum is never `Pod` (most byte patterns are not a variant) and Rust lays
//! one out with alignment padding, so it cannot be a field of a struct that is
//! read by casting account bytes. `ZcEnum<E>` is a plain byte container of
//! alignment 1: `get` decodes the bytes into `E` (failing on an unknown tag or
//! an invalid nested value) and `set` encodes `E` into them.
//!
//! The stored form is packed: the tag (the enum's `repr` integer,
//! little-endian), then the active variant's fields one after another with no
//! padding, then zeros up to the size of the largest variant. [`ZcCodec`]
//! describes that form for one type and is implemented for every plain-data
//! field type here; `#[defined_type]` generates it for enums and structs.

use {
    crate::{error::PodError, unaligned::Unaligned, Bool, FixedString, FixedVec, Opt},
    bytemuck::{Pod, Zeroable},
    core::marker::PhantomData,
};

/// A type that can be a field of a zero-copy struct. Exactly the `Pod` types;
/// the separate trait exists to give a precise error for the common mistakes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a zero-copy struct",
    label = "this type is not `Pod`",
    note = "a `#[defined_type]` enum field must be written `ZcEnum<EnumName>`",
    note = "`bool` fields must be `Bool`; `Vec` and `String` need the Borsh or Wincode representation, or `FixedVec`/`FixedString`"
)]
pub trait ZcField {}

impl<T: Pod> ZcField for T {}

/// A type that can be a field of a zero-copy `#[event(alloc)]`, or the element
/// type of a `Vec`/`Option` field there: exactly the types whose bytes can be
/// written out as they are (`NoUninit`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a zero-copy event",
    label = "this type's bytes cannot be written as they are",
    note = "a `#[defined_type]` enum field must be written `ZcEnum<EnumName>`"
)]
pub trait ZcEventField: bytemuck::NoUninit {}

impl<T: bytemuck::NoUninit> ZcEventField for T {}

/// A direct field of a zero-copy `#[event(alloc)]` must be `Default` for the
/// event's generated `Default` impl. This trait has the single requirement
/// `Default` so a field that fails it reports the message below.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a zero-copy event",
    label = "this type has no default value",
    note = "a `#[defined_type]` enum field must be written `ZcEnum<EnumName>`"
)]
pub trait ZcEventDefault: Default {}

impl<T: Default> ZcEventDefault for T {}

/// The default value of one direct event field, for the generated `Default`.
pub fn event_field_default<T: ZcEventDefault>() -> T {
    T::default()
}

/// The bytes of one event field. Generated `#[event(alloc)]` code writes every
/// field through this, so a field type that cannot be written reports the
/// `ZcEventField` message instead of a raw `NoUninit` error.
pub fn event_bytes_of<T: ZcEventField>(value: &T) -> &[u8] {
    bytemuck::bytes_of(value)
}

/// The bytes of a `Vec` event field's elements, back to back.
pub fn event_slice_bytes<T: ZcEventField>(values: &[T]) -> &[u8] {
    bytemuck::cast_slice(values)
}

/// The packed stored form of a value.
///
/// `encode` writes exactly [`Self::SIZE`] bytes into a slice of that length
/// that is all zero on entry; `decode` reads them back and returns `None`
/// when the bytes are not a valid value. Neither panics on a slice of the
/// wrong length: `encode` writes nothing and `decode` returns `None`.
pub trait ZcCodec: Sized {
    /// Bytes the value occupies when stored.
    const SIZE: usize;

    fn encode(&self, out: &mut [u8]);

    fn decode(bytes: &[u8]) -> Option<Self>;
}

/// Writes the bytes of `value` at the start of `out`, or nothing when `out` is
/// too short. This is `ZcCodec::encode` for a `Pod` type; `#[defined_type]`
/// structs call it.
pub fn encode_pod<T: Pod>(value: &T, out: &mut [u8]) {
    let source = bytemuck::bytes_of(value);
    if let Some(target) = out.get_mut(..source.len()) {
        target.copy_from_slice(source);
    }
}

/// Reads a `T` from the start of `bytes`, or `None` when `bytes` is too short.
/// This is `ZcCodec::decode` for a `Pod` type; `#[defined_type]` structs call
/// it.
pub fn decode_pod<T: Pod>(bytes: &[u8]) -> Option<T> {
    bytemuck::try_pod_read_unaligned(bytes.get(..core::mem::size_of::<T>())?).ok()
}

macro_rules! zc_codec_for_pod {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ZcCodec for $ty {
                const SIZE: usize = core::mem::size_of::<$ty>();

                #[inline(always)]
                fn encode(&self, out: &mut [u8]) {
                    encode_pod(self, out);
                }

                #[inline(always)]
                fn decode(bytes: &[u8]) -> Option<Self> {
                    decode_pod(bytes)
                }
            }
        )*
    };
}

zc_codec_for_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, Bool);

#[cfg(feature = "solana")]
zc_codec_for_pod!(solana_address::Address);

impl ZcCodec for bool {
    const SIZE: usize = 1;

    fn encode(&self, out: &mut [u8]) {
        if let Some(byte) = out.first_mut() {
            *byte = u8::from(*self);
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes.first()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

impl<T: Pod> ZcCodec for Opt<T> {
    const SIZE: usize = core::mem::size_of::<Opt<T>>();

    fn encode(&self, out: &mut [u8]) {
        encode_pod(self, out);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        decode_pod(bytes)
    }
}

impl<T: Pod, const N: usize> ZcCodec for FixedVec<T, N> {
    const SIZE: usize = core::mem::size_of::<FixedVec<T, N>>();

    fn encode(&self, out: &mut [u8]) {
        encode_pod(self, out);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        decode_pod(bytes)
    }
}

impl<const N: usize> ZcCodec for FixedString<N> {
    const SIZE: usize = core::mem::size_of::<FixedString<N>>();

    fn encode(&self, out: &mut [u8]) {
        encode_pod(self, out);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        decode_pod(bytes)
    }
}

impl<T: ZcCodec, const N: usize> ZcCodec for [T; N] {
    const SIZE: usize = T::SIZE * N;

    fn encode(&self, out: &mut [u8]) {
        for (index, item) in self.iter().enumerate() {
            if let Some(slot) = out.get_mut(index * T::SIZE..(index + 1) * T::SIZE) {
                item.encode(slot);
            }
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let mut items: [Option<T>; N] = core::array::from_fn(|_| None);
        for (index, item) in items.iter_mut().enumerate() {
            *item = Some(T::decode(
                bytes.get(index * T::SIZE..(index + 1) * T::SIZE)?,
            )?);
        }
        let mut decoded = items.into_iter();
        Some(core::array::from_fn(|_| {
            decoded.next().flatten().expect("every element was decoded")
        }))
    }
}

/// An enum that can be stored in a zero-copy struct.
///
/// `#[defined_type]` implements this trait together with [`ZcCodec`]; do not
/// implement it by hand. `Stored` must be `ZcStorage<Self, { Self::SIZE }>`;
/// the storage type checks that at compile time wherever it is used.
pub trait ZcEnumKind: ZcCodec + Copy + 'static {
    type Stored;
}

/// The stored form of the enum `E`: `E`'s packed bytes, alignment 1.
#[repr(C)]
pub struct ZcStorage<E, const SIZE: usize> {
    bytes: [u8; SIZE],
    _kind: PhantomData<E>,
}

/// The type of a zero-copy struct field holding the enum `E`.
pub type ZcEnum<E> = <E as ZcEnumKind>::Stored;

impl<E, const SIZE: usize> Clone for ZcStorage<E, SIZE> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E, const SIZE: usize> Copy for ZcStorage<E, SIZE> {}

// SAFETY: every byte pattern is valid and all-zero is valid (`bytes` is
// `[u8; SIZE]`; the other field is zero-sized).
unsafe impl<E: 'static, const SIZE: usize> Zeroable for ZcStorage<E, SIZE> {}

// SAFETY: `repr(C)` over a byte array and a zero-sized field has no padding
// and alignment 1. Every byte pattern is valid (`get` checks them), and the
// type is `Copy` and `'static`.
unsafe impl<E: 'static, const SIZE: usize> Pod for ZcStorage<E, SIZE> {}

// SAFETY: a byte array and a zero-sized field have alignment 1.
unsafe impl<E: 'static, const SIZE: usize> Unaligned for ZcStorage<E, SIZE> {}

impl<E: 'static, const SIZE: usize> Default for ZcStorage<E, SIZE> {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl<E, const SIZE: usize> PartialEq for ZcStorage<E, SIZE> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl<E, const SIZE: usize> Eq for ZcStorage<E, SIZE> {}

impl<E, const SIZE: usize> core::fmt::Debug for ZcStorage<E, SIZE> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ZcStorage").field(&self.bytes).finish()
    }
}

impl<E, const SIZE: usize> ZcStorage<E, SIZE>
where
    E: ZcEnumKind<Stored = Self>,
{
    const MATCHES_ENUM: () = assert!(
        SIZE == E::SIZE,
        "ZcEnum storage does not match the enum's stored size"
    );

    /// Stores `value`.
    pub fn new(value: E) -> Self {
        let mut stored = Self::zeroed();
        stored.set(value);
        stored
    }

    /// Decodes the stored bytes, or fails with `InvalidEnumValue` when they
    /// name no variant or hold an invalid nested value.
    pub fn get(&self) -> Result<E, PodError> {
        let () = Self::MATCHES_ENUM;
        E::decode(&self.bytes).ok_or(PodError::InvalidEnumValue)
    }

    /// Stores `value`, replacing the previous contents.
    pub fn set(&mut self, value: E) {
        let () = Self::MATCHES_ENUM;
        self.bytes.fill(0);
        value.encode(&mut self.bytes);
    }
}

impl<E, const SIZE: usize> From<E> for ZcStorage<E, SIZE>
where
    E: ZcEnumKind<Stored = Self>,
{
    fn from(value: E) -> Self {
        Self::new(value)
    }
}

impl<E, const SIZE: usize> ZcCodec for ZcStorage<E, SIZE>
where
    E: 'static,
{
    const SIZE: usize = SIZE;

    fn encode(&self, out: &mut [u8]) {
        if let Some(target) = out.get_mut(..SIZE) {
            target.copy_from_slice(&self.bytes);
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let mut stored = Self::zeroed();
        stored.bytes.copy_from_slice(bytes.get(..SIZE)?);
        Some(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u8)]
    enum Color {
        Red = 0,
        Green = 1,
        Blue = 7,
    }

    impl ZcCodec for Color {
        const SIZE: usize = 1;

        fn encode(&self, out: &mut [u8]) {
            if let Some(tag) = out.first_mut() {
                *tag = *self as u8;
            }
        }

        fn decode(bytes: &[u8]) -> Option<Self> {
            match bytes.first()? {
                0 => Some(Color::Red),
                1 => Some(Color::Green),
                7 => Some(Color::Blue),
                _ => None,
            }
        }
    }

    impl ZcEnumKind for Color {
        type Stored = ZcStorage<Color, 1>;
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u8)]
    enum Shape {
        Empty = 0,
        Point(u32) = 1,
        Rect { width: u16, height: u64 } = 2,
        Painted(Color) = 3,
        Grid([Color; 2]) = 4,
    }

    impl ZcCodec for Shape {
        // tag + the largest payload (Rect: 2 + 8)
        const SIZE: usize = 1 + 10;

        fn encode(&self, out: &mut [u8]) {
            match *self {
                Shape::Empty => {}
                Shape::Point(x) => {
                    out[0] = 1;
                    x.encode(&mut out[1..5]);
                }
                Shape::Rect { width, height } => {
                    out[0] = 2;
                    width.encode(&mut out[1..3]);
                    height.encode(&mut out[3..11]);
                }
                Shape::Painted(color) => {
                    out[0] = 3;
                    color.encode(&mut out[1..2]);
                }
                Shape::Grid(cells) => {
                    out[0] = 4;
                    cells.encode(&mut out[1..3]);
                }
            }
        }

        fn decode(bytes: &[u8]) -> Option<Self> {
            match *bytes.first()? {
                0 => Some(Shape::Empty),
                1 => Some(Shape::Point(u32::decode(bytes.get(1..5)?)?)),
                2 => Some(Shape::Rect {
                    width: u16::decode(bytes.get(1..3)?)?,
                    height: u64::decode(bytes.get(3..11)?)?,
                }),
                3 => Some(Shape::Painted(Color::decode(bytes.get(1..2)?)?)),
                4 => Some(Shape::Grid(<[Color; 2]>::decode(bytes.get(1..3)?)?)),
                _ => None,
            }
        }
    }

    impl ZcEnumKind for Shape {
        type Stored = ZcStorage<Shape, 11>;
    }

    #[test]
    fn stored_enums_have_alignment_one_and_the_packed_size() {
        assert_eq!(core::mem::size_of::<ZcEnum<Color>>(), 1);
        assert_eq!(core::mem::size_of::<ZcEnum<Shape>>(), 11);
        assert_eq!(core::mem::align_of::<ZcEnum<Color>>(), 1);
        assert_eq!(core::mem::align_of::<ZcEnum<Shape>>(), 1);
    }

    #[test]
    fn a_zeroed_slot_is_the_variant_with_tag_zero() {
        let color: ZcEnum<Color> = Zeroable::zeroed();
        assert_eq!(color.get(), Ok(Color::Red));
        let shape: ZcEnum<Shape> = Zeroable::zeroed();
        assert_eq!(shape.get(), Ok(Shape::Empty));
    }

    #[test]
    fn every_variant_round_trips() {
        for color in [Color::Red, Color::Green, Color::Blue] {
            assert_eq!(ZcEnum::<Color>::new(color).get(), Ok(color));
        }
        for shape in [
            Shape::Empty,
            Shape::Point(0xAABB_CCDD),
            Shape::Rect {
                width: 513,
                height: u64::MAX,
            },
            Shape::Painted(Color::Blue),
            Shape::Grid([Color::Green, Color::Blue]),
        ] {
            let mut stored = ZcEnum::<Shape>::new(shape);
            assert_eq!(stored.get(), Ok(shape));
            stored.set(shape);
            assert_eq!(stored.get(), Ok(shape));
        }
    }

    #[test]
    fn set_writes_the_packed_bytes_and_zeroes_the_rest() {
        let stored = ZcEnum::<Shape>::new(Shape::Point(0xAABB_CCDD));
        assert_eq!(
            bytemuck::bytes_of(&stored),
            &[1, 0xDD, 0xCC, 0xBB, 0xAA, 0, 0, 0, 0, 0, 0]
        );

        let mut stored = ZcEnum::<Shape>::new(Shape::Rect {
            width: 513,
            height: u64::MAX,
        });
        assert_eq!(
            bytemuck::bytes_of(&stored),
            &[2, 1, 2, 255, 255, 255, 255, 255, 255, 255, 255]
        );

        stored.set(Shape::Painted(Color::Blue));
        assert_eq!(
            bytemuck::bytes_of(&stored),
            &[3, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        stored.set(Shape::Empty);
        assert_eq!(bytemuck::bytes_of(&stored), &[0u8; 11]);
    }

    #[test]
    fn an_invalid_tag_or_nested_value_is_an_error_not_undefined_behaviour() {
        let bad: ZcEnum<Color> = bytemuck::cast([2u8]);
        assert_eq!(bad.get(), Err(PodError::InvalidEnumValue));

        let mut bytes = [0u8; 11];
        bytes[0] = 5;
        let bad: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert_eq!(bad.get(), Err(PodError::InvalidEnumValue));

        bytes[0] = 3;
        bytes[1] = 5;
        let bad: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert_eq!(bad.get(), Err(PodError::InvalidEnumValue));
        bytes[1] = 7;
        let good: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert_eq!(good.get(), Ok(Shape::Painted(Color::Blue)));

        bytes[0] = 4;
        bytes[1] = 1;
        bytes[2] = 9;
        let bad: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert_eq!(bad.get(), Err(PodError::InvalidEnumValue));
    }

    #[test]
    fn every_possible_tag_byte_is_accepted_exactly_when_it_names_a_variant() {
        for tag in 0..=255u8 {
            let mut bytes = [0u8; 11];
            bytes[0] = tag;
            let stored: ZcEnum<Shape> = bytemuck::cast(bytes);
            assert_eq!(stored.get().is_ok(), tag <= 4, "tag {tag}");
        }
    }

    #[test]
    fn a_packed_struct_holding_stored_enums_is_pod_and_castable_at_any_address() {
        #[repr(C, packed)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct Holder {
            id: u64,
            shape: ZcEnum<Shape>,
            color: ZcEnum<Color>,
            tail: [u8; 7],
        }

        assert_eq!(core::mem::size_of::<Holder>(), 8 + 11 + 1 + 7);
        let mut buffer = [0u8; 28];
        buffer[1] = 9;
        buffer[9] = 1;
        buffer[10] = 0x78;
        buffer[20] = 7;
        let holder: &Holder = bytemuck::from_bytes(&buffer[1..28]);
        assert_eq!({ holder.id }, 9);
        assert_eq!(holder.shape.get(), Ok(Shape::Point(0x78)));
        assert_eq!(holder.color.get(), Ok(Color::Blue));
    }

    #[test]
    fn codecs_for_arrays_bools_and_options() {
        let mut out = [0u8; 3];
        [Color::Red, Color::Blue, Color::Green].encode(&mut out);
        assert_eq!(out, [0, 7, 1]);
        assert_eq!(
            <[Color; 3]>::decode(&out),
            Some([Color::Red, Color::Blue, Color::Green])
        );
        assert_eq!(<[Color; 3]>::decode(&[0, 2, 1]), None);
        assert_eq!(<[Color; 3]>::decode(&[0, 7]), None);

        assert_eq!(bool::decode(&[0]), Some(false));
        assert_eq!(bool::decode(&[1]), Some(true));
        assert_eq!(bool::decode(&[2]), None);
        let mut flag = [9u8];
        true.encode(&mut flag);
        assert_eq!(flag, [1]);

        let option = Opt::some(0x0102u16);
        let mut bytes = [0u8; 3];
        option.encode(&mut bytes);
        assert_eq!(bytes, [1, 2, 1]);
        assert_eq!(Opt::<u16>::decode(&bytes), Some(option));
    }

    #[test]
    fn codecs_never_panic_on_slices_of_the_wrong_length() {
        let mut empty: [u8; 0] = [];
        0xFFFF_FFFFu32.encode(&mut empty);
        assert_eq!(u32::decode(&[1, 2, 3]), None);
        assert_eq!(<[u8; 4]>::decode(&[1]), None);
        assert_eq!(Shape::decode(&[]), None);
        assert_eq!(Shape::decode(&[2, 0]), None);
    }

    #[test]
    fn stored_enums_compare_by_bytes() {
        let a = ZcEnum::<Shape>::new(Shape::Point(1));
        let b = ZcEnum::<Shape>::new(Shape::Point(1));
        let c = ZcEnum::<Shape>::new(Shape::Point(2));
        assert!(a == b);
        assert!(a != c);
    }
}
