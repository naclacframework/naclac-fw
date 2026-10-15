//! # Zero-Copy Enum Fields
//!
//! `ZcEnum<E>` is how a zero-copy struct stores a `#[defined_type]` enum `E`.
//! An enum is never `Pod` (most byte patterns are not a variant), so it cannot
//! be a field of a struct that is read by casting account or instruction
//! bytes. `ZcEnum<E>` is a plain byte container with exactly `E`'s size and
//! alignment, which is `Pod`; `get` validates the bytes and returns a copy of
//! `E`, and `set` writes `E` without ever copying padding.

use crate::prelude::{bytemuck, Address, Bool, Opt, ProgramError};
use bytemuck::{Pod, Zeroable};
use core::marker::PhantomData;

/// A type that can be a field of a zero-copy struct. Exactly the `Pod` types;
/// the separate trait exists to give a precise error for the common mistakes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a zero-copy struct",
    label = "this type is not `Pod`",
    note = "a `#[defined_type]` enum field must be written `ZcEnum<EnumName>`",
    note = "`bool` fields must be `Bool`; `Vec` and `String` need the Borsh or Wincode representation"
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

/// The alignment of a stored enum, as a type.
pub struct Align<const A: usize>;

/// Maps an alignment to an integer type with that alignment.
pub trait AlignTo {
    type Unit: Pod;
}

impl AlignTo for Align<1> {
    type Unit = u8;
}
impl AlignTo for Align<2> {
    type Unit = u16;
}
impl AlignTo for Align<4> {
    type Unit = u32;
}
impl AlignTo for Align<8> {
    type Unit = u64;
}

/// Writes a value's bytes in zero-copy layout.
///
/// `out` is exactly `size_of::<Self>()` bytes and all zero on entry.
/// Implementations write only the bytes that carry data, never padding.
pub trait ZcWrite {
    fn write_into(&self, out: &mut [u8]);
}

/// Writes one field of an enum variant into its slot in the variant's payload
/// layout. Implemented for every `Pod` type, so the field's `Pod` impl alone
/// decides whether it can be written, whichever crate or version defines the
/// type. A generated enum implements it by delegating to its own `ZcWrite`.
pub trait ZcFieldWrite {
    fn write_field(&self, out: &mut [u8]);
}

impl<T: Pod> ZcFieldWrite for T {
    fn write_field(&self, out: &mut [u8]) {
        out.copy_from_slice(bytemuck::bytes_of(self));
    }
}

macro_rules! impl_zc_write_for_pod {
    ($($ty:ty),* $(,)?) => {
        $(impl ZcWrite for $ty {
            fn write_into(&self, out: &mut [u8]) {
                out.copy_from_slice(bytemuck::bytes_of(self));
            }
        })*
    };
}

impl_zc_write_for_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, Bool, Address);

impl ZcWrite for bool {
    fn write_into(&self, out: &mut [u8]) {
        out[0] = u8::from(*self);
    }
}

impl<T: Pod> ZcWrite for Opt<T> {
    fn write_into(&self, out: &mut [u8]) {
        out.copy_from_slice(bytemuck::bytes_of(self));
    }
}

impl<T: ZcWrite, const N: usize> ZcWrite for [T; N] {
    fn write_into(&self, out: &mut [u8]) {
        let item_size = core::mem::size_of::<T>();
        for (index, item) in self.iter().enumerate() {
            item.write_into(&mut out[index * item_size..(index + 1) * item_size]);
        }
    }
}

/// An enum that can be stored in a zero-copy struct.
///
/// # Safety
/// `Stored` must be `ZcStorage<Self, { align_of::<Self>() }, { size_of::<Self>() }>`,
/// and `write_into` must write a byte pattern that `Self`'s
/// `CheckedBitPattern` accepts and read back as the same value.
/// `#[defined_type]` implements this trait; do not implement it by hand.
pub unsafe trait ZcEnumKind: Copy + 'static + bytemuck::CheckedBitPattern + ZcWrite {
    type Stored;
}

/// The stored form of the enum `E`: `E`'s size and alignment, as bytes.
#[repr(C)]
pub struct ZcStorage<E, const ALIGN: usize, const SIZE: usize>
where
    Align<ALIGN>: AlignTo,
{
    _alignment: [<Align<ALIGN> as AlignTo>::Unit; 0],
    bytes: [u8; SIZE],
    _kind: PhantomData<E>,
}

/// The type of a zero-copy struct field holding the enum `E`.
pub type ZcEnum<E> = <E as ZcEnumKind>::Stored;

impl<E, const ALIGN: usize, const SIZE: usize> Clone for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> Copy for ZcStorage<E, ALIGN, SIZE> where
    Align<ALIGN>: AlignTo
{
}

// SAFETY: every byte pattern is valid and all-zero is valid (`bytes` is
// `[u8; SIZE]`; the other fields are zero-sized).
unsafe impl<E, const ALIGN: usize, const SIZE: usize> Zeroable for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
}

// SAFETY: `ZcEnumKind` guarantees `ALIGN`/`SIZE` are `E`'s alignment and
// size, and a size is always a multiple of its alignment, so `repr(C)` adds
// no padding after `bytes`. The zero-sized fields add none. Every byte
// pattern is valid, and `ZcStorage` is `Copy` and `'static`.
unsafe impl<E, const ALIGN: usize, const SIZE: usize> Pod for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
}

impl<E, const ALIGN: usize, const SIZE: usize> Default for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
    fn default() -> Self {
        Self::zeroed()
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> PartialEq for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
{
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> Eq for ZcStorage<E, ALIGN, SIZE> where
    Align<ALIGN>: AlignTo
{
}

impl<E, const ALIGN: usize, const SIZE: usize> core::fmt::Debug for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ZcStorage").field(&self.bytes).finish()
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
    const MATCHES_ENUM: () = assert!(
        ALIGN == core::mem::align_of::<E>() && SIZE == core::mem::size_of::<E>(),
        "ZcEnum storage does not match the enum's size and alignment"
    );

    /// Stores `value`.
    pub fn new(value: E) -> Self {
        let mut stored = Self::zeroed();
        stored.set(value);
        stored
    }

    /// Validates the stored bytes and returns the enum they hold.
    pub fn get(&self) -> Result<E, ProgramError> {
        let () = Self::MATCHES_ENUM;
        bytemuck::checked::try_pod_read_unaligned::<E>(&self.bytes)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    /// Stores `value`, replacing the previous contents.
    pub fn set(&mut self, value: E) {
        let () = Self::MATCHES_ENUM;
        self.bytes.fill(0);
        value.write_into(&mut self.bytes);
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> From<E> for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
    fn from(value: E) -> Self {
        Self::new(value)
    }
}

impl<E, const ALIGN: usize, const SIZE: usize> ZcWrite for ZcStorage<E, ALIGN, SIZE>
where
    Align<ALIGN>: AlignTo,
    E: ZcEnumKind<Stored = Self>,
{
    fn write_into(&self, out: &mut [u8]) {
        out.copy_from_slice(&self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::CheckedBitPattern;
    use core::mem::{align_of, offset_of, size_of};

    #[derive(Clone, Copy, PartialEq, Eq, Debug, CheckedBitPattern)]
    #[repr(u8)]
    enum Color {
        Red = 0,
        Green = 1,
        Blue = 7,
    }

    impl ZcWrite for Color {
        fn write_into(&self, out: &mut [u8]) {
            out[0] = *self as u8;
        }
    }

    unsafe impl ZcEnumKind for Color {
        type Stored = ZcStorage<Color, 1, 1>;
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug, CheckedBitPattern)]
    #[repr(u8)]
    enum Shape {
        Empty = 0,
        Point(u32) = 1,
        Rect { width: u16, height: u64 } = 2,
        Painted(Color) = 3,
    }

    #[repr(C)]
    struct PointPayload(u8, u32);
    #[repr(C)]
    struct RectPayload(u8, u16, u64);

    impl ZcWrite for Shape {
        fn write_into(&self, out: &mut [u8]) {
            match *self {
                Shape::Empty => {}
                Shape::Point(x) => {
                    out[0] = 1;
                    let at = offset_of!(PointPayload, 1);
                    x.write_into(&mut out[at..at + size_of::<u32>()]);
                }
                Shape::Rect { width, height } => {
                    out[0] = 2;
                    let at = offset_of!(RectPayload, 1);
                    width.write_into(&mut out[at..at + size_of::<u16>()]);
                    let at = offset_of!(RectPayload, 2);
                    height.write_into(&mut out[at..at + size_of::<u64>()]);
                }
                Shape::Painted(color) => {
                    out[0] = 3;
                    out[1] = color as u8;
                }
            }
        }
    }

    unsafe impl ZcEnumKind for Shape {
        type Stored = ZcStorage<Shape, 8, 16>;
    }

    #[test]
    fn stored_size_and_alignment_match_the_enum() {
        assert_eq!(size_of::<ZcEnum<Color>>(), size_of::<Color>());
        assert_eq!(align_of::<ZcEnum<Color>>(), align_of::<Color>());
        assert_eq!(size_of::<ZcEnum<Shape>>(), size_of::<Shape>());
        assert_eq!(align_of::<ZcEnum<Shape>>(), align_of::<Shape>());
        assert_eq!(size_of::<Shape>(), 16);
        assert_eq!(align_of::<Shape>(), 8);
    }

    #[test]
    fn a_zeroed_slot_is_the_variant_with_tag_zero() {
        let color: ZcEnum<Color> = Zeroable::zeroed();
        assert_eq!(color.get().unwrap(), Color::Red);
        let shape: ZcEnum<Shape> = Zeroable::zeroed();
        assert_eq!(shape.get().unwrap(), Shape::Empty);
    }

    #[test]
    fn every_variant_round_trips() {
        for color in [Color::Red, Color::Green, Color::Blue] {
            assert_eq!(ZcEnum::<Color>::new(color).get().unwrap(), color);
        }
        for shape in [
            Shape::Empty,
            Shape::Point(0xAABB_CCDD),
            Shape::Rect {
                width: 513,
                height: u64::MAX,
            },
            Shape::Painted(Color::Blue),
        ] {
            let mut stored = ZcEnum::<Shape>::new(shape);
            assert_eq!(stored.get().unwrap(), shape);
            stored.set(shape);
            assert_eq!(stored.get().unwrap(), shape);
        }
    }

    #[test]
    fn set_writes_exact_bytes_and_leaves_padding_zero() {
        let stored = ZcEnum::<Shape>::new(Shape::Point(0xAABB_CCDD));
        assert_eq!(
            bytemuck::bytes_of(&stored),
            &[1, 0, 0, 0, 0xDD, 0xCC, 0xBB, 0xAA, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        let mut stored = ZcEnum::<Shape>::new(Shape::Rect {
            width: 513,
            height: u64::MAX,
        });
        assert_eq!(
            bytemuck::bytes_of(&stored),
            &[2, 0, 1, 2, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255]
        );

        stored.set(Shape::Empty);
        assert_eq!(bytemuck::bytes_of(&stored), &[0u8; 16]);
    }

    #[test]
    fn an_invalid_tag_is_an_error_not_undefined_behaviour() {
        let bad: ZcEnum<Color> = bytemuck::cast([2u8]);
        assert!(bad.get().is_err());
        let bad: ZcEnum<Shape> = bytemuck::cast([4u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(bad.get().is_err());
    }

    #[test]
    fn a_nested_enum_with_an_invalid_value_is_an_error() {
        let mut bytes = [0u8; 16];
        bytes[0] = 3;
        bytes[1] = 5;
        let bad: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert!(bad.get().is_err());
        bytes[1] = 7;
        let good: ZcEnum<Shape> = bytemuck::cast(bytes);
        assert_eq!(good.get().unwrap(), Shape::Painted(Color::Blue));
    }

    #[test]
    fn a_struct_holding_a_stored_enum_is_pod_and_castable() {
        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct Holder {
            id: u64,
            shape: ZcEnum<Shape>,
            color: ZcEnum<Color>,
            tail: [u8; 7],
        }

        let mut holder: Holder = Zeroable::zeroed();
        {
            let bytes = bytemuck::bytes_of_mut(&mut holder);
            bytes[0] = 9;
            bytes[8] = 1;
            bytes[12] = 0x78;
            bytes[24] = 7;
        }
        assert_eq!(holder.id, 9);
        assert_eq!(holder.shape.get().unwrap(), Shape::Point(0x78));
        assert_eq!(holder.color.get().unwrap(), Color::Blue);

        let copied: Holder = bytemuck::pod_read_unaligned(bytemuck::bytes_of(&holder));
        assert_eq!(copied.shape.get().unwrap(), Shape::Point(0x78));
    }

    #[test]
    fn arrays_of_stored_enums_write_element_by_element() {
        let mut out = [0u8; 3];
        [Color::Red, Color::Blue, Color::Green].write_into(&mut out);
        assert_eq!(out, [0, 7, 1]);
    }
}
