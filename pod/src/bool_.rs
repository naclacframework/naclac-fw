//! `Bool`: a boolean stored as one byte, safe to read from arbitrary bytes.
//!
//! A Rust `bool` is only valid as 0 or 1, so a struct read straight from
//! account bytes cannot hold one. `Bool` holds a `u8`, reads any non-zero byte
//! as `true`, and writes exactly 0 or 1.

use {
    bytemuck::{Pod, Zeroable},
    core::{
        cmp::Ordering,
        fmt,
        hash::{Hash, Hasher},
        ops::{BitAnd, BitOr, Not},
    },
};

#[repr(transparent)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub struct Bool(pub u8);

impl Bool {
    pub const TRUE: Bool = Bool(1);
    pub const FALSE: Bool = Bool(0);

    #[inline(always)]
    pub const fn new(value: bool) -> Self {
        Self(value as u8)
    }
}

impl From<u8> for Bool {
    #[inline(always)]
    fn from(byte: u8) -> Self {
        Self(byte)
    }
}

impl From<bool> for Bool {
    #[inline(always)]
    fn from(value: bool) -> Self {
        Self::new(value)
    }
}

impl From<Bool> for bool {
    #[inline(always)]
    fn from(value: Bool) -> Self {
        value.0 != 0
    }
}

/// Reads and writes a flag field with the same call syntax whether it is a plain `bool`
/// (Borsh and Wincode programs) or a [`Bool`] (zero-copy and Pinocchio programs).
pub trait BoolField {
    fn get(&self) -> bool;
    fn set(&mut self, value: bool);
}

impl BoolField for bool {
    #[inline(always)]
    fn get(&self) -> bool {
        *self
    }

    #[inline(always)]
    fn set(&mut self, value: bool) {
        *self = value;
    }
}

impl BoolField for Bool {
    #[inline(always)]
    fn get(&self) -> bool {
        self.0 != 0
    }

    #[inline(always)]
    fn set(&mut self, value: bool) {
        self.0 = u8::from(value);
    }
}

/// Two `Bool`s are equal when they read the same, so `Bool(1) == Bool(2)`.
impl PartialEq for Bool {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        (self.0 != 0) == (other.0 != 0)
    }
}

impl Eq for Bool {}

impl PartialEq<bool> for Bool {
    #[inline(always)]
    fn eq(&self, other: &bool) -> bool {
        (self.0 != 0) == *other
    }
}

impl PartialEq<Bool> for bool {
    #[inline(always)]
    fn eq(&self, other: &Bool) -> bool {
        *self == (other.0 != 0)
    }
}

impl PartialOrd for Bool {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bool {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0 != 0).cmp(&(other.0 != 0))
    }
}

impl Hash for Bool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0 != 0).hash(state);
    }
}

impl Not for Bool {
    type Output = Bool;

    #[inline(always)]
    fn not(self) -> Bool {
        Bool::new(self.0 == 0)
    }
}

impl BitAnd for Bool {
    type Output = Bool;

    #[inline(always)]
    fn bitand(self, rhs: Bool) -> Bool {
        Bool::new(self.0 != 0 && rhs.0 != 0)
    }
}

impl BitOr for Bool {
    type Output = Bool;

    #[inline(always)]
    fn bitor(self, rhs: Bool) -> Bool {
        Bool::new(self.0 != 0 || rhs.0 != 0)
    }
}

impl BitAnd<bool> for Bool {
    type Output = Bool;

    #[inline(always)]
    fn bitand(self, rhs: bool) -> Bool {
        Bool::new(self.0 != 0 && rhs)
    }
}

impl BitOr<bool> for Bool {
    type Output = Bool;

    #[inline(always)]
    fn bitor(self, rhs: bool) -> Bool {
        Bool::new(self.0 != 0 || rhs)
    }
}

impl fmt::Debug for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&(self.0 != 0), f)
    }
}

impl fmt::Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&(self.0 != 0), f)
    }
}

const _: () = assert!(core::mem::size_of::<Bool>() == 1);
const _: () = assert!(core::mem::align_of::<Bool>() == 1);

#[cfg(test)]
mod tests {
    use super::*;

    struct PlainBoolState {
        flag: bool,
    }

    struct BoolState {
        flag: Bool,
    }

    macro_rules! the_same_statements_work_on {
        ($state:expr) => {{
            let mut state = $state;
            state.flag.set(true);
            assert!(state.flag.get());
            let mut seen = false;
            if state.flag.get() {
                seen = true;
            }
            assert!(seen);
            state.flag.set(false);
            assert!(!state.flag.get());
        }};
    }

    #[test]
    fn the_same_statements_work_on_a_plain_bool_field() {
        the_same_statements_work_on!(PlainBoolState { flag: false });
    }

    #[test]
    fn the_same_statements_work_on_a_bool_field() {
        the_same_statements_work_on!(BoolState { flag: Bool(0) });
    }

    #[test]
    fn set_stores_exactly_one_or_zero() {
        let mut flag = Bool(7);
        flag.set(true);
        assert!(flag.0 == 1);
        flag.set(false);
        assert!(flag.0 == 0);
    }

    #[test]
    fn every_byte_reads_as_true_unless_zero() {
        for byte in 0..=255u8 {
            assert_eq!(bool::from(Bool(byte)), byte != 0);
            assert_eq!(Bool(byte).get(), byte != 0);
        }
    }

    #[test]
    fn writing_a_bool_stores_one_or_zero() {
        assert!(Bool::from(true).0 == 1);
        assert!(Bool::from(false).0 == 0);
    }

    #[test]
    fn equality_ordering_and_hash_follow_the_value_not_the_byte() {
        use core::hash::{Hash, Hasher};

        struct Fnv(u64);
        impl Hasher for Fnv {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for byte in bytes {
                    self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
                }
            }
        }
        let hash = |value: Bool| {
            let mut hasher = Fnv(0xcbf2_9ce4_8422_2325);
            value.hash(&mut hasher);
            hasher.finish()
        };

        assert!(Bool(1) == Bool(2));
        assert!(Bool(0) != Bool(1));
        assert_eq!(Bool(1).cmp(&Bool(200)), Ordering::Equal);
        assert_eq!(Bool(0).cmp(&Bool(5)), Ordering::Less);
        assert_eq!(hash(Bool(1)), hash(Bool(200)));
        assert_ne!(hash(Bool(0)), hash(Bool(1)));
        assert!(Bool(9) == true);
        assert!(false == Bool(0));
    }

    #[test]
    fn operators_produce_canonical_bytes() {
        assert_eq!((!Bool(0)).0, 1);
        assert_eq!((!Bool(77)).0, 0);
        assert_eq!((Bool(3) & Bool(9)).0, 1);
        assert_eq!((Bool(3) & Bool(0)).0, 0);
        assert_eq!((Bool(0) | Bool(200)).0, 1);
        assert_eq!((Bool(0) | Bool(0)).0, 0);
        assert_eq!((Bool(5) & true).0, 1);
        assert_eq!((Bool(0) | false).0, 0);
    }

    #[test]
    fn casting_arbitrary_bytes_is_valid() {
        let bytes: [u8; 4] = [0, 1, 2, 255];
        let flags: &[Bool; 4] = bytemuck::cast_ref(&bytes);
        assert!(!flags[0].get());
        assert!(flags[1].get());
        assert!(flags[2].get());
        assert!(flags[3].get());
    }
}
