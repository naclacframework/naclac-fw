//! # Zero-Copy Spans
//!
//! Provides Span and ZcString, the strictly zero-allocation pointer types used
//! to parse slices and strings out of instruction data in Pinocchio/Zero-Copy environments.

use crate::prelude::{NaclacError, NaclacPod, Pod, Result};
use core::marker::PhantomData;
use core::ops::Deref;

/// A Zero-Copy view into a slice of data (The "Span Revolution").
///
/// A zero-allocation alternative to `Vec<T>` for *instruction arguments* only
/// (write `ZcVec<T>`, an alias for this type, in a zero-copy program's
/// instruction signature to opt in) — it offers a similar API (`.len()`,
/// `.is_empty()`, `Deref<Target = [T]>`) but is just a raw pointer + length
/// into the current instruction's byte buffer, used and discarded within that
/// same call. NOT valid for persisted `#[component]` account storage: a
/// pointer written into on-chain bytes in one transaction is meaningless (or
/// attacker-controlled) when read back in a later one, so the `#[component]`
/// macro rejects `Vec<T>`/`String` fields outright rather than rewriting them
/// to this type.
#[derive(Clone, Copy)]
pub struct Span<T: Pod> {
    ptr: *const T,
    len: usize,
    _phantom: PhantomData<T>,
}

impl<T: Pod> Span<T> {
    /// Creates a new Span from a raw slice.
    pub fn new(data: &[T]) -> Self {
        Self {
            ptr: data.as_ptr(),
            len: data.len(),
            _phantom: PhantomData,
        }
    }

    /// Creates a Span from raw bytes, performing bounds and alignment checks.
    ///
    /// The alignment check is not optional: `as_slice()`/`Deref` build a `&[T]`
    /// via `core::slice::from_raw_parts`, which requires the pointer to already
    /// be aligned for `T` — an unaligned `&[T]` is undefined behavior in Rust
    /// even if it happens to work on the target hardware. Since this is called
    /// at a byte offset that's the running sum of every preceding instruction
    /// argument's raw size (no padding inserted between them), that offset is
    /// not guaranteed to satisfy `align_of::<T>()` for `T` wider than a byte.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let len = data.len();
        let item_size = core::mem::size_of::<T>();

        if !len.is_multiple_of(item_size) {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }

        if !(data.as_ptr() as usize).is_multiple_of(core::mem::align_of::<T>()) {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }

        let count = len / item_size;

        Ok(Self {
            ptr: data.as_ptr() as *const T,
            len: count,
            _phantom: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &'static [T] {
        // SAFETY: The pointer and length were validated upon Span creation bounds checking.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Explicitly convert to a heap-allocated Vec (Borsh-compatibility).
    #[cfg(not(feature = "pinocchio"))]
    pub fn to_vec(&self) -> crate::prelude::Vec<T> {
        self.as_slice().to_vec()
    }
}

impl<T: Pod> Deref for Span<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: Pod> core::fmt::Debug for Span<T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_slice().iter()).finish()
    }
}

/// A Zero-Copy view into a UTF-8 string.
///
/// This is the zero-copy in-place replacement for `String`: the `#[component]`
/// macro rewrites `String` fields to `ZcString` in zero-copy mode. Backed by a
/// `Span<u8>` into the account's existing bytes — no heap allocation or copying.
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
        unsafe { core::str::from_utf8_unchecked(self.inner.as_slice()) }
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
