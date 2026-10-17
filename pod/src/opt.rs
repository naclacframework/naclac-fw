//! `Opt<T>`: an optional plain-data value stored as a tag byte followed by the
//! value, `#[repr(C, packed)]`, so it is `1 + size_of::<T>()` bytes with no
//! padding and alignment 1.
//!
//! The tag is 1 for a value and 0 for none. A stored tag of 2 to 255 is not
//! written by this type; reads treat it as none, and [`Opt::tag_is_valid`]
//! reports it. The value bytes of a none are always zero when the value was
//! built or changed through this type.

use {
    crate::unaligned::Unaligned,
    bytemuck::{Pod, Zeroable},
    core::fmt,
};

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Opt<T: Pod> {
    tag: u8,
    value: T,
}

// SAFETY: `repr(C, packed)` over a `u8` and a `Pod` field has no padding, and
// every byte pattern is a valid value (a tag other than 0 or 1 reads as none).
unsafe impl<T: Pod> Pod for Opt<T> {}
// SAFETY: all-zero bytes are a none with a zero value.
unsafe impl<T: Pod> Zeroable for Opt<T> {}
// SAFETY: `repr(packed)` gives alignment 1 whatever `T` is.
unsafe impl<T: Pod> Unaligned for Opt<T> {}

impl<T: Pod> Opt<T> {
    #[inline(always)]
    pub fn some(value: T) -> Self {
        Self { tag: 1, value }
    }

    #[inline(always)]
    pub fn none() -> Self {
        Self::zeroed()
    }

    #[inline(always)]
    pub fn is_some(&self) -> bool {
        self.tag == 1
    }

    #[inline(always)]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// The stored tag byte.
    #[inline(always)]
    pub fn raw_tag(&self) -> u8 {
        self.tag
    }

    /// False when the stored tag is neither 0 nor 1.
    #[inline(always)]
    pub fn tag_is_valid(&self) -> bool {
        self.tag <= 1
    }

    #[inline(always)]
    pub fn get(&self) -> Option<T> {
        if self.is_some() {
            Some(self.value)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn set(&mut self, value: Option<T>) {
        *self = Self::from(value);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        *self = Self::none();
    }

    /// Leaves none behind and returns what was there.
    #[inline(always)]
    pub fn take(&mut self) -> Option<T> {
        let previous = self.get();
        self.clear();
        previous
    }

    /// Stores `value` and returns what was there.
    #[inline(always)]
    pub fn replace(&mut self, value: T) -> Option<T> {
        let previous = self.get();
        *self = Self::some(value);
        previous
    }

    #[inline(always)]
    pub fn unwrap_or(&self, default: T) -> T {
        self.get().unwrap_or(default)
    }

    #[inline(always)]
    pub fn map_or<U>(&self, default: U, f: impl FnOnce(T) -> U) -> U {
        self.get().map_or(default, f)
    }
}

impl<T: Pod> Default for Opt<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: Pod> From<Option<T>> for Opt<T> {
    #[inline(always)]
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::some(value),
            None => Self::none(),
        }
    }
}

impl<T: Pod> From<Opt<T>> for Option<T> {
    #[inline(always)]
    fn from(value: Opt<T>) -> Self {
        value.get()
    }
}

impl<T: Pod + PartialEq> PartialEq for Opt<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Pod + Eq> Eq for Opt<T> {}

impl<T: Pod + fmt::Debug> fmt::Debug for Opt<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_a_tag_then_the_value_with_no_padding() {
        assert_eq!(core::mem::size_of::<Opt<u8>>(), 2);
        assert_eq!(core::mem::size_of::<Opt<u16>>(), 3);
        assert_eq!(core::mem::size_of::<Opt<u32>>(), 5);
        assert_eq!(core::mem::size_of::<Opt<u64>>(), 9);
        assert_eq!(core::mem::size_of::<Opt<u128>>(), 17);
        assert_eq!(core::mem::size_of::<Opt<[u8; 32]>>(), 33);
        assert_eq!(core::mem::align_of::<Opt<u128>>(), 1);
        assert_eq!(core::mem::align_of::<Opt<u64>>(), 1);
    }

    #[test]
    fn bytes_are_the_tag_then_the_little_endian_value() {
        let some = Opt::some(0x0102_0304_0506_0708u64);
        assert_eq!(bytemuck::bytes_of(&some), &[1, 8, 7, 6, 5, 4, 3, 2, 1][..]);
        let none = Opt::<u64>::none();
        assert_eq!(bytemuck::bytes_of(&none), &[0u8; 9][..]);
    }

    #[test]
    fn get_set_take_replace_clear() {
        let mut slot = Opt::<u64>::none();
        assert!(slot.is_none());
        assert_eq!(slot.get(), None);

        slot.set(Some(5));
        assert_eq!(slot.get(), Some(5));
        assert_eq!(slot.replace(9), Some(5));
        assert_eq!(slot.get(), Some(9));
        assert_eq!(slot.take(), Some(9));
        assert!(slot.is_none());
        assert_eq!(slot.take(), None);

        slot.set(Some(1));
        slot.clear();
        assert_eq!(bytemuck::bytes_of(&slot), &[0u8; 9][..]);
        assert_eq!(slot.unwrap_or(7), 7);
        assert_eq!(slot.map_or(0, |v| v + 1), 0);
        slot.set(Some(1));
        assert_eq!(slot.map_or(0, |v| v + 1), 2);
    }

    #[test]
    fn a_none_has_zero_value_bytes() {
        let mut slot = Opt::some(u64::MAX);
        slot.set(None);
        assert_eq!(bytemuck::bytes_of(&slot), &[0u8; 9][..]);
    }

    #[test]
    fn an_unknown_tag_reads_as_none_and_is_reported() {
        let bytes = [2u8, 1, 0, 0, 0];
        let slot: Opt<u32> = bytemuck::pod_read_unaligned(&bytes);
        assert!(slot.is_none());
        assert_eq!(slot.get(), None);
        assert!(!slot.tag_is_valid());
        assert_eq!(slot.raw_tag(), 2);
        assert!(Opt::some(1u32).tag_is_valid());
        assert!(Opt::<u32>::none().tag_is_valid());
    }

    #[test]
    fn equality_ignores_stale_value_bytes_of_a_none() {
        let a: Opt<u32> = bytemuck::pod_read_unaligned(&[0u8, 9, 9, 9, 9]);
        let b = Opt::<u32>::none();
        assert!(a == b);
        assert!(Opt::some(3u32) == Opt::some(3u32));
        assert!(Opt::some(3u32) != Opt::some(4u32));
        assert!(Opt::some(3u32) != b);
    }

    #[test]
    fn conversions_with_option() {
        let from_some: Opt<u16> = Some(4).into();
        let from_none: Opt<u16> = None.into();
        assert_eq!(Option::<u16>::from(from_some), Some(4));
        assert_eq!(Option::<u16>::from(from_none), None);
        let back: Option<u16> = from_some.into();
        assert_eq!(back, Some(4));
        assert!(Opt::<u16>::default().is_none());
    }

    #[test]
    fn nested_in_a_larger_byte_buffer_at_an_odd_offset() {
        let mut buffer = [0u8; 12];
        let slot: &mut Opt<u64> = bytemuck::from_bytes_mut(&mut buffer[1..10]);
        slot.set(Some(0xAABB_CCDD_EEFF_0011));
        assert_eq!(buffer[1], 1);
        assert_eq!(&buffer[2..10], &0xAABB_CCDD_EEFF_0011u64.to_le_bytes());
    }
}
