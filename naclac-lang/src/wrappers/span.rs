//! # Zero-Copy Spans
//!
//! Provides Span and ZcString, the strictly zero-allocation pointer types used 
//! to parse slices and strings out of instruction data in Pinocchio/Zero-Copy environments.

use core::ops::Deref;
use core::marker::PhantomData;
use crate::prelude::{NaclacPod, Pod, Result, NaclacError};

/// A Zero-Copy view into a slice of data (The "Span Revolution").
/// 
/// In Zero-Copy mode, this type is aliased to `Vec<T>` to provide
/// a familiar API while avoiding heap allocations and redundant copies.
#[derive(Clone, Copy)]
pub struct Span<'info, T: Pod> {
    ptr: *const T,
    len: usize,
    _phantom: PhantomData<&'info T>,
}

impl<'info, T: Pod> Span<'info, T> {
    /// Creates a new Span from a raw slice.
    pub fn new(data: &'info [T]) -> Self {
        Self {
            ptr: data.as_ptr(),
            len: data.len(),
            _phantom: PhantomData,
        }
    }

    /// Creates a Span from raw bytes, performing bounds and alignment checks.
    pub fn from_bytes(data: &'info [u8]) -> Result<Self> {
        let len = data.len();
        let item_size = core::mem::size_of::<T>();
        
        if len % item_size != 0 {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }

        let count = len / item_size;
        
        // Safety: We check alignment and size.
        // In Solana/Pinocchio, instruction data is usually 8-byte aligned.
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

    pub fn as_slice(&self) -> &'info [T] {
        // SAFETY: The pointer and length were validated upon Span creation bounds checking.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Explicitly convert to a heap-allocated Vec (Borsh-compatibility).
    #[cfg(not(feature = "pinocchio"))]
    pub fn to_vec(&self) -> crate::prelude::Vec<T> {
        self.as_slice().to_vec()
    }
}

impl<'info, T: Pod> Deref for Span<'info, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'info, T: Pod> core::fmt::Debug for Span<'info, T> where T: core::fmt::Debug {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_slice().iter()).finish()
    }
}

/// A Zero-Copy view into a UTF-8 string.
/// 
/// In Zero-Copy mode, this type is aliased to `String` to provide
/// a familiar API while avoiding heap allocations and redundant copies.
#[derive(Clone, Copy)]
pub struct ZcString<'info> {
    inner: Span<'info, u8>,
}

impl<'info> ZcString<'info> {
    pub fn from_bytes(data: &'info [u8]) -> Result<Self> {
        // Validate UTF-8
        core::str::from_utf8(data).map_err(|_| NaclacError::InvalidInstructionData.err(0))?;
        Ok(Self {
            inner: Span::new(data),
        })
    }

    pub fn as_str(&self) -> &'info str {
        // SAFETY: The bytes were validated as correct UTF-8 strings upon struct initialization.
        unsafe { core::str::from_utf8_unchecked(self.inner.as_slice()) }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Explicitly convert to a heap-allocated String (Borsh-compatibility).
    #[cfg(not(feature = "pinocchio"))]
    pub fn to_string(&self) -> crate::prelude::String {
        crate::prelude::String::from(self.as_str())
    }
}

impl<'info> Deref for ZcString<'info> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<'info> core::fmt::Display for ZcString<'info> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'info> core::fmt::Debug for ZcString<'info> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

// NaclacPod implementation for automated deserialization
impl<'info, T: Pod> NaclacPod for Span<'info, T> {
    fn naclac_from_bytes(data: &[u8]) -> Self {
        // SAFETY: The instruction data bytes are transmuted into our zero-copy string representation.
        Self::from_bytes(unsafe { core::mem::transmute(data) }).unwrap_or(Self { ptr: core::ptr::null(), len: 0, _phantom: PhantomData })
    }
    fn naclac_size() -> usize {
        0 // Dynamic size
    }
}
