// ===========================================================================
// view.rs — borrowed, allocation-free views over fixed-size records
// ===========================================================================

//! Metaplex Core stores several plugin payloads as a count followed by
//! fixed-size records. `RecordList` reads such a run in place from the
//! account bytes. It depends on no account-data layout feature, so it is
//! available to every program regardless of whether its own accounts use
//! `bytemuck`, `borsh` or `wincode`.

use {
    crate::prelude::{Address, NaclacError, Result},
    core::marker::PhantomData,
};

/// A value stored as exactly `SIZE` consecutive bytes with no alignment
/// requirement.
pub trait Record: Sized {
    const SIZE: usize;

    /// Decodes one record. `bytes.len()` is always exactly `SIZE`.
    fn decode(bytes: &[u8]) -> Self;
}

impl Record for u8 {
    const SIZE: usize = 1;

    fn decode(bytes: &[u8]) -> Self {
        bytes.first().copied().unwrap_or(0)
    }
}

impl Record for Address {
    const SIZE: usize = 32;

    fn decode(bytes: &[u8]) -> Self {
        let mut array = [0u8; 32];
        for (slot, byte) in array.iter_mut().zip(bytes) {
            *slot = *byte;
        }
        Address::new_from_array(array)
    }
}

/// A run of `T` records borrowed from account bytes for `'a`. Element access
/// copies the record out; no reference to a `T` is ever formed, so the
/// underlying bytes need no particular alignment.
pub struct RecordList<'a, T: Record> {
    data: &'a [u8],
    _record: PhantomData<T>,
}

impl<'a, T: Record> RecordList<'a, T> {
    /// An empty list.
    pub fn empty() -> Self {
        Self {
            data: &[],
            _record: PhantomData,
        }
    }

    /// Views `data` as consecutive records. Fails unless `data.len()` is a
    /// whole multiple of `T::SIZE`.
    pub fn from_bytes(data: &'a [u8]) -> Result<Self> {
        if T::SIZE == 0 || !data.len().is_multiple_of(T::SIZE) {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        Ok(Self {
            data,
            _record: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.data.len() / T::SIZE
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<T> {
        let start = index.checked_mul(T::SIZE)?;
        let end = start.checked_add(T::SIZE)?;
        Some(T::decode(self.data.get(start..end)?))
    }

    pub fn iter(&self) -> RecordIter<'a, T> {
        RecordIter {
            list: *self,
            index: 0,
        }
    }

    pub fn to_vec(&self) -> crate::prelude::Vec<T> {
        self.iter().collect()
    }
}

impl<'a> RecordList<'a, u8> {
    /// A byte run needs no length check, since every length is a whole number
    /// of one-byte records.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            data: bytes,
            _record: PhantomData,
        }
    }

    /// The viewed bytes, borrowed for the full `'a`.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

pub struct RecordIter<'a, T: Record> {
    list: RecordList<'a, T>,
    index: usize,
}

impl<T: Record> Iterator for RecordIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let item = self.list.get(self.index);
        if item.is_some() {
            self.index += 1;
        }
        item
    }
}

impl<T: Record> Clone for RecordList<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Record> Copy for RecordList<'_, T> {}

impl<T: Record + core::fmt::Debug> core::fmt::Debug for RecordList<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Record + PartialEq> PartialEq for RecordList<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<T: Record + Eq> Eq for RecordList<'_, T> {}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves building a `RecordList<Address>` over any prefix of any buffer,
    /// and reading any index from it, never panics, and that an accepted
    /// list's length times the record size is exactly the byte length.
    #[kani::proof]
    fn prove_record_list_access_never_panics() {
        let data: [u8; 66] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= data.len());
        let index: usize = kani::any();
        if let Ok(list) = RecordList::<Address>::from_bytes(&data[..len]) {
            assert_eq!(list.len() * Address::SIZE, len);
            let _ = list.get(index);
        }
    }
}
