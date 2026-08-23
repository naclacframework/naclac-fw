//! # Zero-Copy Spans
//!
//! Provides Span and ZcString, the strictly zero-allocation pointer types used
//! to parse slices and strings out of instruction data in Pinocchio/Zero-Copy environments.

use crate::prelude::{bytemuck, NaclacError, NaclacPod, Pod, Result};
use core::marker::PhantomData;
use core::ops::Deref;

/// A Zero-Copy view into a slice of data (The "Span Revolution").
///
/// A zero-allocation alternative to `Vec<T>` for *instruction arguments* only
/// (write `ZcVec<T>`, an alias for this type, in a zero-copy program's
/// instruction signature to opt in) — it offers a similar API (`.len()`,
/// `.is_empty()`, `.iter()`, `.get()`) but is just a raw pointer + length into
/// the current instruction's byte buffer, used and discarded within that same
/// call. NOT valid for persisted `#[component]` account storage: a pointer
/// written into on-chain bytes in one transaction is meaningless (or
/// attacker-controlled) when read back in a later one, so the `#[component]`
/// macro rejects `Vec<T>`/`String` fields outright rather than rewriting them
/// to this type.
///
/// Element access always goes through an unaligned read
/// (`bytemuck::pod_read_unaligned`), never a real `&T`/`&[T]` reference: an
/// instruction-data buffer is a tightly packed byte stream with no alignment
/// padding inserted between arguments (each argument's offset is just the
/// running sum of every preceding argument's raw size), so a `T` wider than
/// one byte is not guaranteed to land on a `T`-aligned address. Building a
/// real `&[T]` there via `core::slice::from_raw_parts` would be undefined
/// behavior regardless of whether the bytes happen to be valid.
#[derive(Clone, Copy)]
pub struct Span<T: Pod> {
    ptr: *const u8,
    len: usize,
    _phantom: PhantomData<T>,
}

impl<T: Pod> Span<T> {
    /// Creates a new Span from a raw slice.
    pub fn new(data: &[T]) -> Self {
        Self {
            ptr: data.as_ptr() as *const u8,
            len: data.len(),
            _phantom: PhantomData,
        }
    }

    /// Creates a Span from raw bytes, performing a bounds check only — no
    /// alignment check, since element access never requires `T`-alignment
    /// (see this type's own doc comment).
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let len = data.len();
        let item_size = core::mem::size_of::<T>();

        if item_size == 0 || !len.is_multiple_of(item_size) {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }

        Ok(Self {
            ptr: data.as_ptr(),
            len: len / item_size,
            _phantom: PhantomData,
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

    pub fn iter(&self) -> SpanIter<'_, T> {
        SpanIter { span: self, idx: 0 }
    }

    /// Explicitly convert to a heap-allocated Vec (Borsh-compatibility).
    #[cfg(not(feature = "pinocchio"))]
    pub fn to_vec(&self) -> crate::prelude::Vec<T> {
        self.iter().collect()
    }
}

/// Yields elements by value (unaligned reads — see [`Span`]'s doc comment),
/// not by reference: unlike a real slice iterator, there is no `&T` to hand
/// out without first materializing a properly aligned copy.
pub struct SpanIter<'a, T: Pod> {
    span: &'a Span<T>,
    idx: usize,
}

impl<T: Pod> Iterator for SpanIter<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let v = self.span.get(self.idx);
        if v.is_some() {
            self.idx += 1;
        }
        v
    }
}

/// `u8` has no alignment requirement — any pointer is valid for a `&[u8]`
/// reference regardless of how this `Span` was constructed, so this one
/// element type gets a real zero-copy byte-slice accessor.
impl Span<u8> {
    pub fn as_bytes(&self) -> &'static [u8] {
        // SAFETY: u8's alignment requirement is 1, always satisfied; `ptr`
        // covers `len` bytes (see `get`'s SAFETY note).
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T: Pod + core::fmt::Debug> core::fmt::Debug for Span<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// A Zero-Copy view into a UTF-8 string.
///
/// This is the zero-copy in-place replacement for `String` — but, like
/// `Span<T>` above, only ever for *instruction arguments*, parsed fresh out
/// of the current instruction's byte buffer and discarded at the end of the
/// call (`naclac-macros/src/accounts.rs`'s `is_zc_string` codegen branch).
///
/// **Correction:** an earlier version of this doc claimed "the `#[component]`
/// macro rewrites `String` fields to `ZcString` in zero-copy mode." That is
/// not what the macro does, and never has been: `naclac-macros/src/component.rs`'s
/// zero-copy branch explicitly *rejects* `String`/`Vec<T>` `#[component]`
/// fields with a compile error rather than rewriting them, precisely because
/// a `Span`/`ZcString` is a raw pointer + length — sound only for data that
/// lives entirely within one instruction's call frame, never for persisted
/// account bytes read back in a later transaction (see `component.rs`'s error
/// message, which instead points authors at a fixed-size array like
/// `[u8; 64]` for persisted string-shaped data).
/// Backed by a `Span<u8>` into the instruction data — no heap allocation or
/// copying.
#[derive(Clone, Copy)]
pub struct ZcString {
    inner: Span<u8>,
}

impl ZcString {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        // Validate UTF-8
        core::str::from_utf8(data).map_err(|_| NaclacError::InvalidInstructionData.err(0))?;
        Ok(Self {
            inner: Span::new(data),
        })
    }

    pub fn as_str(&self) -> &'static str {
        // SAFETY: The bytes were validated as correct UTF-8 strings upon struct initialization.
        unsafe { core::str::from_utf8_unchecked(self.inner.as_bytes()) }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Deref for ZcString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl core::fmt::Display for ZcString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl core::fmt::Debug for ZcString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

// NaclacPod implementation for automated deserialization
impl<T: Pod> NaclacPod for Span<T> {
    fn naclac_from_bytes(data: &[u8]) -> Self {
        Self::from_bytes(data).unwrap_or(Self {
            ptr: core::ptr::null(),
            len: 0,
            _phantom: PhantomData,
        })
    }
    fn naclac_size() -> usize {
        0 // Dynamic size
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests — direct, allocation-free exercise of `Span<T>`/`ZcString`.
//
// This is the first coverage this file has ever had (confirmed while
// building `tests/dup-mut`'s `write_note` instruction, the first program in
// the repo to actually use a `ZcString` instruction argument). Deliberately
// placed here rather than routed entirely through a `tests/*` litesvm case:
// while writing that case it became clear `naclac-idl`/`naclac-client-gen`
// have no support for dynamic-length (`String`/`ZcString`/`Vec<T>`)
// instruction arguments at all — `naclac-client-gen/src/rust/instructions/offchain.rs`
// always serializes an instruction's args as one fixed lump, either
// `bytemuck::bytes_of(&args)` (zero-copy/pinocchio) or a single
// `BorshSerialize` call (borsh) — and a struct containing a `String`/`ZcString`
// field can't derive `bytemuck::Pod` at all, so the generated client for any
// such instruction would likely fail to compile in zero-copy/pinocchio mode.
// That's a real, previously-unknown gap (flagged in `tests/TEST_PLAN.md`),
// separate from whether `Span`/`ZcString` themselves work correctly — these
// tests answer that second question directly and confidently, without
// depending on the uncertain client-codegen path at all.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_from_bytes_round_trips_exact_values() {
        let values: [u32; 4] = [1, 2, 3, 4_000_000_000];
        let bytes = crate::prelude::bytemuck::cast_slice::<u32, u8>(&values);
        let span = Span::<u32>::from_bytes(bytes).expect("well-sized bytes must parse");
        assert_eq!(span.len(), 4);
        assert!(!span.is_empty());
        let collected: crate::prelude::Vec<u32> = span.iter().collect();
        assert_eq!(collected, values);
        for (i, v) in values.iter().enumerate() {
            assert_eq!(span.get(i), Some(*v));
        }
        assert_eq!(span.get(4), None);
    }

    #[test]
    fn span_from_bytes_rejects_size_not_a_multiple_of_item_size() {
        // 5 raw bytes can't divide evenly into any whole number of `u32`s (4
        // bytes each) — must be rejected, not silently truncated.
        let bytes: [u8; 5] = [0, 1, 2, 3, 4];
        let result = Span::<u32>::from_bytes(&bytes);
        assert!(
            result.is_err(),
            "a non-multiple byte length must be rejected"
        );
    }

    #[test]
    fn span_from_bytes_accepts_and_reads_correctly_regardless_of_alignment() {
        // Deliberately offset by 1 byte so a 4-byte-aligned type never gets a
        // 4-byte-aligned pointer, regardless of where the backing allocation
        // itself happens to start — instruction-data buffers offer no such
        // guarantee either, so `Span` must work correctly either way.
        let values: [u32; 2] = [0x11223344, 0xAABBCCDD];
        let mut backing = [0u8; 9];
        backing[1..9].copy_from_slice(bytemuck::cast_slice::<u32, u8>(&values));
        let slice = &backing[1..9];
        assert_ne!(
            (slice.as_ptr() as usize) % core::mem::align_of::<u32>(),
            0,
            "test setup must actually produce a misaligned pointer"
        );

        let span = Span::<u32>::from_bytes(slice).expect("misaligned bytes must still parse");
        assert_eq!(span.get(0), Some(values[0]));
        assert_eq!(span.get(1), Some(values[1]));
    }

    #[test]
    fn zc_string_round_trips_exact_utf8_bytes() {
        let original = "hello zero-copy world! \u{1F980}"; // includes a multi-byte codepoint
        let s = ZcString::from_bytes(original.as_bytes()).expect("valid UTF-8 must parse");
        assert_eq!(s.len(), original.len());
        assert!(!s.is_empty());
        assert_eq!(s.as_str(), original);
        assert_eq!(&*s, original);
        assert_eq!(format!("{}", s), original);
    }

    #[test]
    fn zc_string_rejects_invalid_utf8() {
        // 0xFF is never valid as a UTF-8 lead byte.
        let invalid: [u8; 3] = [0xFF, 0x00, 0x01];
        let result = ZcString::from_bytes(&invalid);
        assert!(
            result.is_err(),
            "invalid UTF-8 bytes must be rejected, not passed through"
        );
    }

    #[test]
    fn zc_string_empty_is_empty() {
        let s = ZcString::from_bytes(&[]).expect("empty input is valid UTF-8");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.as_str(), "");
    }
}
