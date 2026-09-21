// ===========================================================================
// fixed_buf.rs — stack-allocated CPI instruction data buffer (pinocchio only)
// ===========================================================================

//! `FixedBuf<N>` — the data-bytes counterpart to `solana-instruction-view`'s
//! own const-generic account arrays (`invoke_signed::<const ACCOUNTS: usize,
//! _>`, which stack-allocates `[MaybeUninit<CpiAccount>; ACCOUNTS]` rather
//! than heap-allocating, verified directly in that crate's real source).
//! Every CPI instruction this crate builds on the pinocchio backend has an
//! exact, compile-time-computable maximum data length (a discriminator byte
//! plus a small number of fixed/bounded-width fields), so `N` is sized per
//! call site to that exact maximum — never a guess, never a shared "big
//! enough" constant.
//!
//! `push`/`extend` panic on overflow rather than returning `Result`: a
//! caller providing `N` sized correctly for its own encoding can never
//! overflow, so a panic here means the crate's own size arithmetic is wrong,
//! not that the caller supplied bad input — the same class of bug a raw
//! `buf[i..j].copy_from_slice(..)` would already panic on.

#![cfg(feature = "pinocchio")]

/// Backs the shared `encode_*` helpers (`plugin_authority::encode_plugin_authority`,
/// `external_plugin_adapter::encode_key`/`encode_linked_data_key`) so each
/// call site can choose its own buffer strategy: a `FixedBuf<N>` when the
/// total size is bounded at compile time, or a heap `Vec<u8>` for the rare
/// case where it genuinely isn't (arbitrary caller-supplied content, e.g.
/// `write_asset_external_adapter_signed`'s inline data bytes).
pub(crate) trait ByteSink {
    fn push(&mut self, byte: u8);
    fn extend_from_slice(&mut self, bytes: &[u8]);
}

impl ByteSink for crate::prelude::Vec<u8> {
    fn push(&mut self, byte: u8) {
        crate::prelude::Vec::push(self, byte);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        crate::prelude::Vec::extend_from_slice(self, bytes);
    }
}

pub(crate) struct FixedBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FixedBuf<N> {
    pub(crate) fn new() -> Self {
        Self { buf: [0u8; N], len: 0 }
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl<const N: usize> ByteSink for FixedBuf<N> {
    fn push(&mut self, byte: u8) {
        self.buf[self.len] = byte;
        self.len += 1;
    }

    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `push` never panics and correctly accumulates bytes, in
    /// order, for any sequence of pushes that stays within `N`'s capacity —
    /// exactly the precondition every real call site guarantees via a
    /// compile-time-sized `N` (see this module's own doc comment). A wrong
    /// `self.len` bookkeeping step would either panic early or silently
    /// write to the wrong offset while still "not panicking" for a shorter
    /// sequence — this checks both, not just the panic-freedom half.
    #[kani::proof]
    fn prove_fixed_buf_push_within_capacity_is_correct() {
        const N: usize = 4;
        let mut buf: FixedBuf<N> = FixedBuf::new();
        let bytes: [u8; N] = kani::any();
        for &b in bytes.iter() {
            buf.push(b);
        }
        assert_eq!(buf.as_slice(), &bytes[..]);
    }

    /// Same property for a single `extend_from_slice` call of any length up
    /// to `N` — proves the offset arithmetic (`self.len..self.len +
    /// bytes.len()`) never reads/writes out of bounds and writes the exact
    /// bytes at the exact offset.
    ///
    /// `#[kani::unwind(N + 2)]`: `take` is a *symbolic* length (only
    /// range-constrained via `kani::assume`, not a compile-time constant),
    /// so CBMC's automatic loop-unwinding-bound inference can't derive a
    /// tight stopping point for the `memcmp` inside `assert_eq!`'s slice
    /// comparison — without an explicit bound it keeps trying larger and
    /// larger unwind depths instead of stopping at the `take <= N` bound
    /// `kani::assume` already established. `N + 2` is a safety margin over
    /// the real max (`N`); if it's ever too tight, Kani reports an explicit
    /// "unwinding assertion" failure telling us to raise it, rather than
    /// silently proving something weaker than intended.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_fixed_buf_extend_from_slice_within_capacity_is_correct() {
        const N: usize = 8;
        let mut buf: FixedBuf<N> = FixedBuf::new();
        let bytes: [u8; N] = kani::any();
        let take: usize = kani::any();
        kani::assume(take <= N);
        buf.extend_from_slice(&bytes[..take]);
        assert_eq!(buf.as_slice(), &bytes[..take]);
    }

    /// Proves the realistic call pattern every real `encode_*` helper in
    /// this crate uses — a discriminator byte via `push`, then payload
    /// bytes via `extend_from_slice` — never panics and lands at the right
    /// offsets when the combined length fits `N`, i.e. that the two
    /// methods' length bookkeeping composes correctly across calls, not
    /// just in isolation.
    /// See `prove_fixed_buf_extend_from_slice_within_capacity_is_correct`'s
    /// doc comment for why an explicit unwind bound is needed here too —
    /// same symbolic-length `take`, same `memcmp`-unwinding cause.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_fixed_buf_push_then_extend_within_capacity_is_correct() {
        const N: usize = 8;
        let mut buf: FixedBuf<N> = FixedBuf::new();
        let first: u8 = kani::any();
        buf.push(first);
        let rest: [u8; N - 1] = kani::any();
        let take: usize = kani::any();
        kani::assume(take <= N - 1);
        buf.extend_from_slice(&rest[..take]);
        assert_eq!(buf.as_slice()[0], first);
        assert_eq!(&buf.as_slice()[1..], &rest[..take]);
    }
}
