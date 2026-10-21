// ===========================================================================
// read.rs — bounds-checked little-endian reads from raw account bytes
// ===========================================================================

//! Every Metaplex Core field is read from untrusted account bytes. These
//! helpers return `AccountDataTooSmall` when the requested range does not lie
//! inside `data`, and contain no panic path, so callers propagate with `?`
//! instead of slicing and converting by hand.

use crate::{
    checked_end,
    prelude::{Address, NaclacError, Result},
};

/// The `N` bytes of `data` starting at `offset`.
pub(crate) fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = checked_end(offset, N)?;
    let bytes = data
        .get(offset..end)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    let mut out = [0u8; N];
    for (slot, byte) in out.iter_mut().zip(bytes) {
        *slot = *byte;
    }
    Ok(out)
}

pub(crate) fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(data, offset)?))
}

pub(crate) fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(data, offset)?))
}

pub(crate) fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(data, offset)?))
}

pub(crate) fn read_address(data: &[u8], offset: usize) -> Result<Address> {
    Ok(Address::new_from_array(read_array(data, offset)?))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves every reader returns `Ok` or `Err` and never panics, for any
    /// buffer contents and any `offset` including values whose end would
    /// overflow `usize`.
    #[kani::proof]
    fn prove_readers_never_panic() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = read_u16(&data, offset);
        let _ = read_u32(&data, offset);
        let _ = read_u64(&data, offset);
        let _ = read_address(&data, offset);
    }
}
