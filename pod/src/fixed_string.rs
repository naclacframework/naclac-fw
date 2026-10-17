//! `FixedString<N>`: a UTF-8 string of at most `N` bytes stored inline as a
//! two-byte little-endian length followed by all `N` byte slots.
//!
//! It is a [`FixedVec<u8, N>`](crate::FixedVec) with string operations, so it
//! has alignment 1, no padding, and size `2 + N`. `N` counts bytes, not
//! characters. The bytes are not trusted: reading the text checks them as
//! UTF-8 every time and fails with `InvalidUtf8`, and the writing methods only
//! ever store valid UTF-8.

use {
    crate::{error::PodError, fixed_vec::FixedVec, unaligned::Unaligned},
    bytemuck::{Pod, Zeroable},
    core::fmt,
};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct FixedString<const N: usize> {
    bytes: FixedVec<u8, N>,
}

// SAFETY: `repr(transparent)` over a `Pod` type.
unsafe impl<const N: usize> Pod for FixedString<N> {}
// SAFETY: all-zero bytes are the empty string.
unsafe impl<const N: usize> Zeroable for FixedString<N> {}
// SAFETY: `repr(transparent)` over a type of alignment 1.
unsafe impl<const N: usize> Unaligned for FixedString<N> {}

impl<const N: usize> FixedString<N> {
    /// An empty string.
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            bytes: FixedVec::new(),
        }
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Length in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// The text, or `InvalidUtf8` when the stored bytes are not valid UTF-8.
    pub fn as_str(&self) -> Result<&str, PodError> {
        core::str::from_utf8(self.as_bytes()).map_err(|_| PodError::InvalidUtf8)
    }

    /// Replaces the contents with `value`, or fails with `CapacityExceeded` and
    /// leaves the string unchanged when `value` is longer than `N` bytes.
    pub fn try_set(&mut self, value: &str) -> Result<(), PodError> {
        self.bytes.try_set_from_slice(value.as_bytes())
    }

    /// Appends `value`, or fails with `CapacityExceeded` and leaves the string
    /// unchanged when it does not fit.
    pub fn try_push_str(&mut self, value: &str) -> Result<(), PodError> {
        self.bytes.try_extend_from_slice(value.as_bytes())
    }

    /// Shortens the string to at most `new_len` bytes, rounding down to the
    /// nearest character boundary so no character is cut, and zeroes the freed
    /// bytes. Does nothing when `new_len` is not below the length.
    pub fn truncate(&mut self, new_len: usize) {
        let bytes = self.as_bytes();
        if new_len >= bytes.len() {
            return;
        }
        let mut cut = new_len;
        while cut > 0
            && bytes
                .get(cut)
                .is_some_and(|byte| byte & 0b1100_0000 == 0b1000_0000)
        {
            cut -= 1;
        }
        self.bytes.truncate(cut);
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
    }

    /// True when the string is exactly `other`.
    pub fn eq_str(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> PartialEq for FixedString<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for FixedString<N> {}

impl<const N: usize> PartialEq<str> for FixedString<N> {
    fn eq(&self, other: &str) -> bool {
        self.eq_str(other)
    }
}

impl<const N: usize> PartialEq<&str> for FixedString<N> {
    fn eq(&self, other: &&str) -> bool {
        self.eq_str(other)
    }
}

impl<const N: usize> fmt::Debug for FixedString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Ok(text) => fmt::Debug::fmt(text, f),
            Err(_) => fmt::Debug::fmt(self.as_bytes(), f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_a_two_byte_length_then_the_bytes() {
        assert_eq!(core::mem::size_of::<FixedString<10>>(), 12);
        assert_eq!(core::mem::size_of::<FixedString<0>>(), 2);
        assert_eq!(core::mem::align_of::<FixedString<10>>(), 1);
        let mut name = FixedString::<4>::new();
        name.try_set("ab").unwrap_or_else(|_| panic!("set"));
        assert_eq!(bytemuck::bytes_of(&name), &[2, 0, b'a', b'b', 0, 0][..]);
    }

    #[test]
    fn set_push_and_read_back() {
        let mut name = FixedString::<8>::new();
        assert!(name.is_empty());
        name.try_set("héllo").unwrap_or_else(|_| panic!("set"));
        assert_eq!(name.as_str(), Ok("héllo"));
        assert_eq!(name.len(), 6);
        name.try_push_str("!").unwrap_or_else(|_| panic!("push"));
        assert_eq!(name.as_str(), Ok("héllo!"));
        assert!(name.try_push_str("abc") == Err(PodError::CapacityExceeded));
        assert_eq!(name.as_str(), Ok("héllo!"));
        assert!(name.try_set("123456789") == Err(PodError::CapacityExceeded));
        assert_eq!(name.as_str(), Ok("héllo!"));
    }

    #[test]
    fn n_counts_bytes_not_characters() {
        let mut name = FixedString::<4>::new();
        assert!(name.try_set("éé").is_ok());
        assert!(name.try_set("ééé") == Err(PodError::CapacityExceeded));
        assert!(name.try_set("😀").is_ok());
        assert!(name.try_set("😀a") == Err(PodError::CapacityExceeded));
    }

    #[test]
    fn invalid_utf8_is_reported_not_trusted() {
        let mut bytes = [0u8; 2 + 8];
        bytes[0] = 2;
        bytes[2] = 0xFF;
        bytes[3] = 0xFE;
        let name: &FixedString<8> = bytemuck::from_bytes(&bytes);
        assert!(name.as_str() == Err(PodError::InvalidUtf8));
        assert_eq!(name.as_bytes(), &[0xFF, 0xFE]);
        assert_eq!(std::format!("{name:?}"), "[255, 254]");
    }

    #[test]
    fn truncate_never_cuts_a_character() {
        let mut name = FixedString::<16>::new();
        name.try_set("aé😀b").unwrap_or_else(|_| panic!("set"));
        assert_eq!(name.len(), 1 + 2 + 4 + 1);

        let mut cut = name;
        cut.truncate(5);
        assert_eq!(cut.as_str(), Ok("aé"));
        let mut cut = name;
        cut.truncate(2);
        assert_eq!(cut.as_str(), Ok("a"));
        let mut cut = name;
        cut.truncate(3);
        assert_eq!(cut.as_str(), Ok("aé"));
        let mut cut = name;
        cut.truncate(7);
        assert_eq!(cut.as_str(), Ok("aé😀"));
        let mut cut = name;
        cut.truncate(0);
        assert_eq!(cut.as_str(), Ok(""));
        let mut cut = name;
        cut.truncate(100);
        assert_eq!(cut.as_str(), Ok("aé😀b"));
    }

    #[test]
    fn freed_bytes_are_zeroed_and_equal_strings_have_equal_bytes() {
        let mut a = FixedString::<8>::new();
        a.try_set("abcdef").unwrap_or_else(|_| panic!("set"));
        a.truncate(2);
        let mut b = FixedString::<8>::new();
        b.try_set("ab").unwrap_or_else(|_| panic!("set"));
        assert_eq!(bytemuck::bytes_of(&a), bytemuck::bytes_of(&b));
        assert!(a == b);
        a.clear();
        assert!(bytemuck::bytes_of(&a).iter().all(|byte| *byte == 0));
    }

    #[test]
    fn comparison_with_str() {
        let mut name = FixedString::<8>::new();
        name.try_set("abc").unwrap_or_else(|_| panic!("set"));
        assert!(name == "abc");
        assert!(name != "abd");
        assert!(name.eq_str("abc"));
        assert!(name != *"ab");
    }

    #[test]
    fn works_at_an_odd_address() {
        let mut buffer = [0u8; 16];
        let name: &mut FixedString<8> = bytemuck::from_bytes_mut(&mut buffer[3..13]);
        name.try_set("hi").unwrap_or_else(|_| panic!("set"));
        assert_eq!(&buffer[3..7], &[2, 0, b'h', b'i']);
    }
}
