//! # Zero-Copy Views
//!
//! `ZcVec` and `ZcString` are the strictly zero-allocation pointer types used
//! to parse slices and strings out of instruction data in Pinocchio/Zero-Copy environments.

use {
    crate::error::PodError,
    bytemuck::Pod,
    core::{marker::PhantomData, ops::Deref},
};

/// A zero-copy view into a slice of data.
///
/// A zero-allocation alternative to `Vec<T>` for *instruction arguments* only
/// (write `ZcVec<T>` in a zero-copy program's instruction signature to opt
/// in) — it offers a similar API (`.len()`,
/// `.is_empty()`, `.iter()`, `.get()`) but is just a raw pointer + length into
/// the current instruction's byte buffer, used and discarded within that same
/// call. NOT valid for persisted `#[component]` account storage: a pointer
/// written into on-chain bytes in one transaction is meaningless (or
/// attacker-controlled) when read back in a later one, so the `#[component]`
/// macro rejects `ZcVec<T>`/`ZcString` fields outright.
///
/// Element access always goes through an unaligned read
/// (`bytemuck::pod_read_unaligned`), never a real `&T`/`&[T]` reference: an
/// instruction-data buffer is a tightly packed byte stream with no alignment
/// padding inserted between arguments (each argument's offset is just the
/// running sum of every preceding argument's raw size), so a `T` wider than
/// one byte is not guaranteed to land on a `T`-aligned address. Building a
/// real `&[T]` there via `core::slice::from_raw_parts` would be undefined
/// behavior regardless of whether the bytes happen to be valid.
///
/// `'a` is the lifetime of the bytes the view was built from; a view, and
/// everything it hands out, cannot outlive them.
#[derive(Clone, Copy)]
pub struct ZcVec<'a, T: Pod> {
    ptr: *const u8,
    len: usize,
    _borrow: PhantomData<&'a [T]>,
}

impl<'a, T: Pod> ZcVec<'a, T> {
    /// Creates a new ZcVec from a raw slice.
    pub fn new(data: &'a [T]) -> Self {
        Self {
            ptr: data.as_ptr() as *const u8,
            len: data.len(),
            _borrow: PhantomData,
        }
    }

    /// Creates a ZcVec from raw bytes, performing a bounds check only — no
    /// alignment check, since element access never requires `T`-alignment
    /// (see this type's own doc comment).
    pub fn from_bytes(data: &'a [u8]) -> Result<Self, PodError> {
        let len = data.len();
        let item_size = core::mem::size_of::<T>();

        if item_size == 0 || !len.is_multiple_of(item_size) {
            return Err(PodError::InvalidInstructionData);
        }

        Ok(Self {
            ptr: data.as_ptr(),
            len: len / item_size,
            _borrow: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reads element `i` by value via an unaligned load.
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len {
            return None;
        }
        let item_size = core::mem::size_of::<T>();
        // SAFETY: `i < self.len`, and `ptr` covers `self.len * item_size`
        // bytes (validated by `from_bytes`'s length check, or guaranteed by
        // `new`'s real `&[T]` origin).
        let bytes = unsafe { core::slice::from_raw_parts(self.ptr.add(i * item_size), item_size) };
        Some(bytemuck::pod_read_unaligned(bytes))
    }

    pub fn iter(&self) -> ZcVecIter<'a, T> {
        ZcVecIter { vec: *self, idx: 0 }
    }

    /// Explicitly convert to a heap-allocated Vec (Borsh-compatibility).
    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> alloc::vec::Vec<T> {
        self.iter().collect()
    }
}

/// Yields elements by value (unaligned reads — see [`ZcVec`]'s doc comment),
/// not by reference: unlike a real slice iterator, there is no `&T` to hand
/// out without first materializing a properly aligned copy.
pub struct ZcVecIter<'a, T: Pod> {
    vec: ZcVec<'a, T>,
    idx: usize,
}

impl<T: Pod> Iterator for ZcVecIter<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let v = self.vec.get(self.idx);
        if v.is_some() {
            self.idx += 1;
        }
        v
    }
}

/// `u8` has no alignment requirement — any pointer is valid for a `&[u8]`
/// reference regardless of how this `ZcVec` was constructed, so this one
/// element type gets a real zero-copy byte-slice accessor.
impl<'a> ZcVec<'a, u8> {
    pub fn as_bytes(&self) -> &'a [u8] {
        // SAFETY: u8's alignment requirement is 1, always satisfied; `ptr`
        // covers `len` bytes (see `get`'s SAFETY note), borrowed for `'a`.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T: Pod + core::fmt::Debug> core::fmt::Debug for ZcVec<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Pod + PartialEq> PartialEq for ZcVec<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<T: Pod + Eq> Eq for ZcVec<'_, T> {}

/// A zero-copy view into a UTF-8 string.
///
/// This is the zero-copy in-place replacement for `String` — but, like
/// `ZcVec<T>` above, only ever for *instruction arguments*, parsed fresh out
/// of the current instruction's byte buffer and discarded at the end of the
/// call. It is never valid for persisted `#[component]` storage: a
/// `ZcVec`/`ZcString` is a raw pointer + length, sound only for data that
/// lives entirely within one instruction's call frame; use `FixedString<N>`
/// for stored text.
/// Backed by a `ZcVec<u8>` into the instruction data — no heap allocation or
/// copying.
#[derive(Clone, Copy)]
pub struct ZcString<'a> {
    inner: ZcVec<'a, u8>,
}

impl<'a> ZcString<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Result<Self, PodError> {
        core::str::from_utf8(data).map_err(|_| PodError::InvalidInstructionData)?;
        Ok(Self {
            inner: ZcVec::new(data),
        })
    }

    pub fn as_str(&self) -> &'a str {
        // SAFETY: The bytes were validated as correct UTF-8 strings upon struct initialization.
        unsafe { core::str::from_utf8_unchecked(self.inner.as_bytes()) }
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.inner.as_bytes()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Deref for ZcString<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl core::fmt::Display for ZcString<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}

impl core::fmt::Debug for ZcString<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zc_vec_from_bytes_round_trips_exact_values() {
        let values: [u32; 4] = [1, 2, 3, 4_000_000_000];
        let bytes = bytemuck::cast_slice::<u32, u8>(&values);
        let view = ZcVec::<u32>::from_bytes(bytes).expect("well-sized bytes must parse");
        assert_eq!(view.len(), 4);
        assert!(!view.is_empty());
        let collected: std::vec::Vec<u32> = view.iter().collect();
        assert_eq!(collected, values);
        for (i, v) in values.iter().enumerate() {
            assert_eq!(view.get(i), Some(*v));
        }
        assert_eq!(view.get(4), None);
    }

    #[test]
    fn zc_vec_partial_eq_compares_by_value_not_by_pointer() {
        let values: [u32; 3] = [7, 8, 9];
        let bytes = bytemuck::cast_slice::<u32, u8>(&values);
        let mut other_backing = [0u8; 12];
        other_backing.copy_from_slice(bytes);

        let a = ZcVec::<u32>::from_bytes(bytes).expect("well-sized bytes must parse");
        let b = ZcVec::<u32>::from_bytes(&other_backing).expect("well-sized bytes must parse");
        assert_ne!(
            a.ptr, b.ptr,
            "test setup must use two distinct backing allocations"
        );
        assert_eq!(a, b);

        let different: [u32; 3] = [7, 8, 10];
        let c = ZcVec::<u32>::from_bytes(bytemuck::cast_slice::<u32, u8>(&different))
            .expect("well-sized bytes must parse");
        assert_ne!(a, c);

        let shorter: [u32; 2] = [7, 8];
        let d = ZcVec::<u32>::from_bytes(bytemuck::cast_slice::<u32, u8>(&shorter))
            .expect("well-sized bytes must parse");
        assert_ne!(a, d, "different lengths must not compare equal");
    }

    #[test]
    fn zc_vec_from_bytes_rejects_size_not_a_multiple_of_item_size() {
        let bytes: [u8; 5] = [0, 1, 2, 3, 4];
        assert!(ZcVec::<u32>::from_bytes(&bytes).err() == Some(PodError::InvalidInstructionData));
    }

    #[test]
    fn zc_vec_from_bytes_accepts_and_reads_correctly_regardless_of_alignment() {
        #[repr(align(4))]
        struct Aligned([u8; 12]);

        let values: [u32; 2] = [0x11223344, 0xAABBCCDD];
        let mut backing = Aligned([0u8; 12]);
        backing.0[1..9].copy_from_slice(bytemuck::cast_slice::<u32, u8>(&values));
        let slice = &backing.0[1..9];
        assert_ne!(
            (slice.as_ptr() as usize) % core::mem::align_of::<u32>(),
            0,
            "test setup must actually produce a misaligned pointer"
        );

        let view = ZcVec::<u32>::from_bytes(slice).expect("misaligned bytes must still parse");
        assert_eq!(view.get(0), Some(values[0]));
        assert_eq!(view.get(1), Some(values[1]));
    }

    #[test]
    fn zc_vec_of_packed_structs_reads_back() {
        #[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        #[repr(C, packed)]
        struct Entry {
            id: u64,
            weight: u16,
        }
        let entries = [Entry { id: 5, weight: 1 }, Entry { id: 6, weight: 2 }];
        let view = ZcVec::new(&entries);
        assert_eq!(view.get(1), Some(Entry { id: 6, weight: 2 }));
        assert_eq!(view.iter().count(), 2);
    }

    #[test]
    fn zc_string_round_trips_exact_utf8_bytes() {
        let original = "hello zero-copy world! \u{1F980}";
        let s = ZcString::from_bytes(original.as_bytes()).expect("valid UTF-8 must parse");
        assert_eq!(s.len(), original.len());
        assert!(!s.is_empty());
        assert_eq!(s.as_str(), original);
        assert_eq!(&*s, original);
        assert_eq!(s.as_bytes(), original.as_bytes());
        assert_eq!(std::format!("{s}"), original);
    }

    #[test]
    fn zc_string_rejects_invalid_utf8() {
        let invalid: [u8; 3] = [0xFF, 0x00, 0x01];
        assert!(ZcString::from_bytes(&invalid).err() == Some(PodError::InvalidInstructionData));
    }

    #[test]
    fn zc_string_empty_is_empty() {
        let s = ZcString::from_bytes(&[]).expect("empty input is valid UTF-8");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.as_str(), "");
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    macro_rules! zc_vec_get_never_panics_harness {
        ($name:ident, $ty:ty, $buf_len:expr) => {
            #[kani::proof]
            fn $name() {
                let data: [u8; $buf_len] = kani::any();
                if let Ok(view) = ZcVec::<$ty>::from_bytes(&data) {
                    let i: usize = kani::any();
                    let _ = view.get(i);
                }
            }
        };
    }
    zc_vec_get_never_panics_harness!(prove_zc_vec_u8_get_never_panics, u8, 8);
    zc_vec_get_never_panics_harness!(prove_zc_vec_u16_get_never_panics, u16, 8);
    zc_vec_get_never_panics_harness!(prove_zc_vec_u32_get_never_panics, u32, 16);
    zc_vec_get_never_panics_harness!(prove_zc_vec_u64_get_never_panics, u64, 24);

    #[kani::proof]
    fn prove_zc_vec_u32_get_reads_correct_bytes() {
        let data: [u8; 16] = kani::any();
        if let Ok(view) = ZcVec::<u32>::from_bytes(&data) {
            let i: usize = kani::any();
            if i < view.len() {
                let got = view.get(i).unwrap();
                let expected = bytemuck::pod_read_unaligned::<u32>(&data[i * 4..i * 4 + 4]);
                assert!(got == expected);
            } else {
                assert!(view.get(i).is_none());
            }
        }
    }

    #[kani::proof]
    fn prove_zc_vec_u32_from_bytes_rejects_non_multiple_length() {
        let data: [u8; 15] = kani::any();
        assert!(ZcVec::<u32>::from_bytes(&data).is_err());
    }

    #[kani::proof]
    fn prove_zc_vec_u8_as_bytes_round_trips() {
        let data: [u8; 8] = kani::any();
        let view = ZcVec::<u8>::from_bytes(&data).unwrap();
        assert!(view.as_bytes() == &data);
    }

    #[kani::proof]
    fn prove_zc_vec_u32_iter_yields_exactly_len_elements() {
        let data: [u8; 16] = kani::any();
        if let Ok(view) = ZcVec::<u32>::from_bytes(&data) {
            let mut count = 0usize;
            for (i, v) in view.iter().enumerate() {
                assert!(Some(v) == view.get(i));
                count += 1;
            }
            assert!(count == view.len());
        }
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn prove_zc_string_from_bytes_matches_str_validation() {
        let data: [u8; 6] = kani::any();
        let is_valid_utf8 = core::str::from_utf8(&data).is_ok();
        assert!(ZcString::from_bytes(&data).is_ok() == is_valid_utf8);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn prove_zc_string_as_str_matches_input_bytes() {
        let data: [u8; 6] = kani::any();
        if let Ok(s) = ZcString::from_bytes(&data) {
            assert!(s.as_str().as_bytes() == &data);
        }
    }
}
