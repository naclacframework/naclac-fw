// ===========================================================================
// cpi.rs — Standardized Cross-Program Invocation (CPI) module
// ===========================================================================

//! # Cross-Program Invocation (CPI) Execution
//!
//! Provides the borrow-checked execution helpers for issuing Cross-Program Invocations.
//! This handles the compilation targets for Solana (via AccountInfo conversion)
//! and Pinocchio (via zero-overhead transmutations).
//!
//! `invoke`/`invoke_signed` (solana) and `invoke_pinocchio`/
//! `invoke_signed_pinocchio_handles`/`invoke_signed_pinocchio` (pinocchio)
//! always resolve to the *checked* path: on solana, [`sdk::invoke_signed`]
//! (the same checks as `solana_cpi::invoke_signed`), which verifies every
//! named account's `AccountInfo` `RefCell` is borrowable before invoking;
//! on pinocchio, the real `solana_instruction_view::cpi::invoke_signed_with_bounds`,
//! which verifies each account's address matches the instruction's expected
//! account at that position and that its borrow state is compatible with the
//! instruction's declared mutability. Both the checked and `_unchecked`
//! pinocchio functions reject (rather than silently truncating) an account
//! list longer than `MAX_CPI_ACCOUNTS` — that bound is a bookkeeping limit,
//! not a borrow-safety one, so skipping it isn't part of what "unchecked"
//! means here. The `_unchecked` variants skip only the borrow/address
//! validation, for callers who have already established the accounts'
//! borrow-safety themselves and want to skip that CU cost — never what
//! naclac's own CPI-caller codegen generates.

/// The seeds of one PDA signer, bump included, held in the form the runtime's
/// CPI syscall reads (`pinocchio::cpi::Seed` on pinocchio, `&[u8]` on the
/// Solana runtime), so signing a CPI with them copies nothing. A PDA takes at
/// most `MAX_PDA_SEEDS` seeds; more fails to compile.
pub struct SignerSeeds<'a, const N: usize> {
    #[cfg(feature = "pinocchio")]
    seeds: [::pinocchio::cpi::Seed<'a>; N],
    #[cfg(not(feature = "pinocchio"))]
    seeds: [&'a [u8]; N],
}

impl<'a, const N: usize> SignerSeeds<'a, N> {
    #[inline(always)]
    pub fn new(parts: [&'a [u8]; N]) -> Self {
        const {
            assert!(
                N <= crate::prelude::MAX_PDA_SEEDS,
                "a PDA takes at most 16 seeds"
            )
        };
        #[cfg(feature = "pinocchio")]
        {
            Self {
                seeds: parts.map(::pinocchio::cpi::Seed::from),
            }
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            Self { seeds: parts }
        }
    }

    /// The signer a `*_signed` CPI takes, borrowing these seeds.
    #[inline(always)]
    pub fn signer(&self) -> CpiSigner<'a, '_> {
        #[cfg(feature = "pinocchio")]
        {
            CpiSigner {
                inner: ::pinocchio::cpi::Signer::from(&self.seeds[..]),
            }
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            CpiSigner { inner: &self.seeds }
        }
    }
}

/// One PDA signer of a signed CPI, borrowing a [`SignerSeeds`]. It has the
/// exact layout of the runtime's own signer entry, so a `&[CpiSigner]` is
/// handed to the CPI syscall as it is.
#[repr(transparent)]
pub struct CpiSigner<'a, 'b> {
    #[cfg(feature = "pinocchio")]
    inner: ::pinocchio::cpi::Signer<'a, 'b>,
    #[cfg(not(feature = "pinocchio"))]
    inner: &'b [&'a [u8]],
}

impl<'a, 'b> CpiSigner<'a, 'b> {
    /// `signers` as pinocchio's own signer slice.
    #[cfg(feature = "pinocchio")]
    #[inline(always)]
    pub fn as_pinocchio<'s>(
        signers: &'s [CpiSigner<'a, 'b>],
    ) -> &'s [::pinocchio::cpi::Signer<'a, 'b>] {
        // SAFETY: `CpiSigner` is `repr(transparent)` over `pinocchio::cpi::Signer`.
        unsafe {
            core::slice::from_raw_parts(
                signers.as_ptr() as *const ::pinocchio::cpi::Signer<'a, 'b>,
                signers.len(),
            )
        }
    }

    /// `signers` as the `&[&[&[u8]]]` the Solana runtime's CPI takes.
    #[cfg(not(feature = "pinocchio"))]
    #[inline(always)]
    pub fn as_seed_slices<'s>(signers: &'s [CpiSigner<'a, 'b>]) -> &'s [&'b [&'a [u8]]] {
        // SAFETY: `CpiSigner` is `repr(transparent)` over `&'b [&'a [u8]]`.
        unsafe {
            core::slice::from_raw_parts(signers.as_ptr() as *const &'b [&'a [u8]], signers.len())
        }
    }
}

#[cfg(test)]
mod signer_seeds_tests {
    use super::{CpiSigner, SignerSeeds};

    #[test]
    fn a_signer_is_its_seeds_in_order() {
        let prefix: &[u8] = b"pool";
        let id = 7u64.to_le_bytes();
        let bump = [254u8];
        let seeds = SignerSeeds::new([prefix, &id, &bump]);
        let signers = [seeds.signer()];

        #[cfg(feature = "pinocchio")]
        {
            let runtime = CpiSigner::as_pinocchio(&signers);
            assert!(runtime.len() == 1);
            let parts: [&[u8]; 3] = [&seeds.seeds[0], &seeds.seeds[1], &seeds.seeds[2]];
            assert!(parts == [prefix, &id[..], &bump[..]]);
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            let runtime = CpiSigner::as_seed_slices(&signers);
            assert!(runtime == [&[prefix, &id[..], &bump[..]][..]]);
        }
    }

    #[test]
    fn a_signer_has_the_runtime_layout() {
        #[cfg(feature = "pinocchio")]
        {
            assert!(
                core::mem::size_of::<CpiSigner<'_, '_>>()
                    == core::mem::size_of::<::pinocchio::cpi::Signer<'_, '_>>()
            );
            assert!(
                core::mem::align_of::<CpiSigner<'_, '_>>()
                    == core::mem::align_of::<::pinocchio::cpi::Signer<'_, '_>>()
            );
        }
        #[cfg(not(feature = "pinocchio"))]
        {
            assert!(core::mem::size_of::<CpiSigner<'_, '_>>() == core::mem::size_of::<&[&[u8]]>());
            assert!(
                core::mem::align_of::<CpiSigner<'_, '_>>() == core::mem::align_of::<&[&[u8]]>()
            );
        }
    }
}

/// Unified AccountMeta for cross-backend CPI usage.
#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub address: crate::prelude::Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(address: crate::prelude::Address, is_writable: bool) -> Self {
        Self {
            address,
            is_signer: false,
            is_writable,
        }
    }
    pub fn new_readonly(address: crate::prelude::Address, is_signer: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable: false,
        }
    }
}

/// CPI instruction data written into an `N`-byte stack buffer, with the
/// `push`/`extend_from_slice` surface the generated CPI builders write
/// through. A write that does not fit is recorded instead of truncated, and
/// [`Self::bytes`] then fails, so a partly written payload is never sent.
pub struct CpiDataBuffer<const N: usize> {
    bytes: [u8; N],
    len: usize,
    overflowed: bool,
}

impl<const N: usize> CpiDataBuffer<N> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
            overflowed: false,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }

    #[inline(always)]
    pub fn extend_from_slice(&mut self, src: &[u8]) {
        let start = self.len;
        let Some(end) = start.checked_add(src.len()) else {
            self.overflowed = true;
            return;
        };
        let Some(dst) = self.bytes.get_mut(start..end) else {
            self.overflowed = true;
            return;
        };
        for (d, s) in dst.iter_mut().zip(src) {
            *d = *s;
        }
        self.len = end;
    }

    /// The bytes written so far, or `SerializationFailed` if any write did
    /// not fit in `N` bytes.
    #[inline(always)]
    pub fn bytes(&self) -> crate::prelude::Result<&[u8]> {
        if self.overflowed {
            return Err(crate::prelude::NaclacError::SerializationFailed.into());
        }
        self.bytes
            .get(..self.len)
            .ok_or_else(|| crate::prelude::NaclacError::SerializationFailed.into())
    }
}

impl<const N: usize> Default for CpiDataBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// CPI instruction data whose length is known only at runtime, written into
/// one zeroed heap allocation of exactly that length, freed on drop. Unlike a
/// `Vec` it never grows. [`Self::bytes`] fails unless exactly the allocated
/// length was written, so neither a short nor an overflowing payload is ever
/// sent.
pub struct CpiHeapBuffer {
    ptr: core::ptr::NonNull<u8>,
    cap: usize,
    len: usize,
    overflowed: bool,
}

impl CpiHeapBuffer {
    /// Allocates `len` zeroed bytes, failing with `SerializationFailed` when
    /// the heap cannot provide them.
    #[inline(always)]
    pub fn with_len(len: usize) -> crate::prelude::Result<Self> {
        if len == 0 {
            return Ok(Self {
                ptr: core::ptr::NonNull::dangling(),
                cap: 0,
                len: 0,
                overflowed: false,
            });
        }
        let layout = core::alloc::Layout::from_size_align(len, 1)
            .map_err(|_| crate::prelude::NaclacError::SerializationFailed)?;
        // SAFETY: `layout` has a non-zero size.
        let raw = unsafe { alloc::alloc::alloc_zeroed(layout) };
        let ptr =
            core::ptr::NonNull::new(raw).ok_or(crate::prelude::NaclacError::SerializationFailed)?;
        Ok(Self {
            ptr,
            cap: len,
            len: 0,
            overflowed: false,
        })
    }

    #[inline(always)]
    fn storage(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` is either dangling with `cap == 0`, or owns `cap`
        // zero-initialised bytes allocated in `with_len` and borrowed
        // mutably through `self` here.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.cap) }
    }

    #[inline(always)]
    pub fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }

    #[inline(always)]
    pub fn extend_from_slice(&mut self, src: &[u8]) {
        let start = self.len;
        let Some(end) = start.checked_add(src.len()) else {
            self.overflowed = true;
            return;
        };
        let Some(dst) = self.storage().get_mut(start..end) else {
            self.overflowed = true;
            return;
        };
        for (d, s) in dst.iter_mut().zip(src) {
            *d = *s;
        }
        self.len = end;
    }

    /// Hands every byte not yet written to `write`, which must fill all of
    /// them, and counts them as written once it succeeds.
    #[inline(always)]
    pub fn fill_rest(
        &mut self,
        write: impl FnOnce(&mut [u8]) -> crate::prelude::Result<()>,
    ) -> crate::prelude::Result<()> {
        let start = self.len;
        let cap = self.cap;
        let rest = self
            .storage()
            .get_mut(start..)
            .ok_or(crate::prelude::NaclacError::SerializationFailed)?;
        write(rest)?;
        self.len = cap;
        Ok(())
    }

    /// The written bytes, or `SerializationFailed` unless exactly the
    /// allocated length was written.
    #[inline(always)]
    pub fn bytes(&self) -> crate::prelude::Result<&[u8]> {
        if self.overflowed || self.len != self.cap {
            return Err(crate::prelude::NaclacError::SerializationFailed.into());
        }
        // SAFETY: same allocation as `storage`, all `len == cap` bytes
        // initialised, borrowed shared through `&self`.
        Ok(unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) })
    }
}

impl Drop for CpiHeapBuffer {
    fn drop(&mut self) {
        if self.cap != 0 {
            // SAFETY: allocated in `with_len` with exactly this size and
            // alignment, and freed only here.
            unsafe {
                alloc::alloc::dealloc(
                    self.ptr.as_ptr(),
                    core::alloc::Layout::from_size_align_unchecked(self.cap, 1),
                );
            }
        }
    }
}

/// `discriminator` followed by the wincode encoding of `args`, in one heap
/// allocation sized by `wincode::config::serialized_size`.
#[cfg(feature = "wincode")]
pub fn wincode_cpi_data<T>(discriminator: &[u8], args: &T) -> crate::prelude::Result<CpiHeapBuffer>
where
    T: crate::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>,
{
    let args_len = crate::wincode::config::serialized_size(args, crate::wincode_config::CONFIG)
        .map_err(|_| crate::prelude::NaclacError::SerializationFailed)?;
    let len = usize::try_from(args_len)
        .ok()
        .and_then(|args_len| discriminator.len().checked_add(args_len))
        .ok_or(crate::prelude::NaclacError::SerializationFailed)?;
    let mut buf = CpiHeapBuffer::with_len(len)?;
    buf.extend_from_slice(discriminator);
    buf.fill_rest(|rest| {
        crate::wincode::config::serialize_into(rest, args, crate::wincode_config::CONFIG)
            .map_err(|_| crate::prelude::NaclacError::SerializationFailed.into())
    })?;
    Ok(buf)
}

#[cfg(test)]
mod cpi_heap_buffer_tests {
    use super::CpiHeapBuffer;

    #[test]
    fn exactly_filled_returns_the_bytes() {
        let mut buf = CpiHeapBuffer::with_len(4).ok().unwrap();
        buf.push(1);
        buf.extend_from_slice(&[2, 3]);
        buf.push(4);
        assert!(buf.bytes().ok() == Some(&[1u8, 2, 3, 4][..]));
    }

    #[test]
    fn a_short_payload_is_never_returned() {
        let mut buf = CpiHeapBuffer::with_len(4).ok().unwrap();
        buf.extend_from_slice(&[1, 2, 3]);
        assert!(buf.bytes().is_err());
    }

    #[test]
    fn an_overflowing_payload_is_never_returned() {
        let mut buf = CpiHeapBuffer::with_len(2).ok().unwrap();
        buf.extend_from_slice(&[1, 2, 3]);
        assert!(buf.bytes().is_err());
    }

    #[test]
    fn fill_rest_counts_the_remainder_as_written() {
        let mut buf = CpiHeapBuffer::with_len(3).ok().unwrap();
        buf.push(9);
        let filled = buf.fill_rest(|rest| {
            for (i, b) in rest.iter_mut().enumerate() {
                *b = i as u8 + 1;
            }
            Ok(())
        });
        assert!(filled.is_ok());
        assert!(buf.bytes().ok() == Some(&[9u8, 1, 2][..]));
    }

    #[test]
    fn an_empty_payload_needs_no_allocation() {
        let buf = CpiHeapBuffer::with_len(0).ok().unwrap();
        assert!(buf.bytes().ok() == Some(&[][..]));
    }
}

#[cfg(test)]
mod cpi_data_buffer_tests {
    use super::CpiDataBuffer;

    #[test]
    fn writes_up_to_capacity_in_order() {
        let mut buf = CpiDataBuffer::<5>::new();
        buf.extend_from_slice(&[1, 2]);
        buf.push(3);
        buf.extend_from_slice(&[4, 5]);
        assert!(buf.bytes().ok() == Some(&[1u8, 2, 3, 4, 5][..]));
    }

    #[test]
    fn a_write_past_capacity_fails_instead_of_truncating() {
        let mut buf = CpiDataBuffer::<4>::new();
        buf.extend_from_slice(&[1, 2, 3]);
        buf.extend_from_slice(&[4, 5]);
        assert!(buf.bytes().is_err());
        buf.push(6);
        assert!(buf.bytes().is_err());
    }

    #[test]
    fn nothing_written_is_empty() {
        let buf = CpiDataBuffer::<0>::new();
        assert!(buf.bytes().ok() == Some(&[][..]));
    }
}

pub trait ToAccountMetas {
    fn to_account_metas(&self) -> crate::prelude::Vec<AccountMeta>;
}

pub trait ToAccountInfos {
    fn to_account_infos(&self) -> crate::prelude::Vec<crate::prelude::AccountInfo>;
}

// ===========================================================================
// SOLANA-PROGRAM BACKEND (invoke / invoke_signed)
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub fn invoke(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    sdk::invoke_signed(instruction, &infos, CpiSigner::as_seed_slices(signers))
}

/// Raw, no-borrow-check CPI — bypasses the `RefCell` borrow checks
/// [`sdk::invoke_signed`] performs.
/// See this module's own doc comment for when to reach for this instead.
#[cfg(not(feature = "pinocchio"))]
pub fn invoke_unchecked(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_unchecked(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed_unchecked(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    sdk::invoke_signed_unchecked(instruction, &infos, CpiSigner::as_seed_slices(signers))
}

/// The runtime-facing CPI ABI of the `solana_program` backend, built from
/// borrowed memory instead of the owning `Vec`s
/// `solana_instruction::Instruction` requires (which `invoke_signed`
/// then clones a second time into this very layout). The runtime reads these
/// structs by offset, so their layouts are checked byte for byte against the
/// SDK's `StableInstruction`/`AccountMeta` in this module's tests.
#[cfg(not(feature = "pinocchio"))]
pub(crate) mod stable {
    use solana_account_info::AccountInfo;

    /// Field order and sizes match `solana_stable_layout::StableVec`.
    #[repr(C)]
    pub(crate) struct StableVec<T> {
        addr: u64,
        cap: u64,
        len: u64,
        _marker: core::marker::PhantomData<T>,
    }

    impl<T> StableVec<T> {
        #[inline(always)]
        fn borrowing(slice: &[T]) -> Self {
            Self {
                addr: slice.as_ptr() as u64,
                cap: slice.len() as u64,
                len: slice.len() as u64,
                _marker: core::marker::PhantomData,
            }
        }
    }

    /// Matches `solana_instruction::AccountMeta`'s `repr(C)` layout.
    #[repr(C)]
    pub(crate) struct StableAccountMeta {
        pub(crate) pubkey: solana_address::Address,
        pub(crate) is_signer: bool,
        pub(crate) is_writable: bool,
    }

    /// Matches `solana_stable_layout::StableInstruction`. Borrows `accounts`
    /// and `data` for `'a`, so it cannot outlive the memory it points into.
    #[repr(C)]
    pub(crate) struct StableInstruction<'a> {
        accounts: StableVec<StableAccountMeta>,
        data: StableVec<u8>,
        program_id: solana_address::Address,
        _borrows: core::marker::PhantomData<&'a ()>,
    }

    impl<'a> StableInstruction<'a> {
        #[inline(always)]
        pub(crate) fn new(
            program_id: solana_address::Address,
            accounts: &'a [StableAccountMeta],
            data: &'a [u8],
        ) -> Self {
            Self {
                accounts: StableVec::borrowing(accounts),
                data: StableVec::borrowing(data),
                program_id,
                _borrows: core::marker::PhantomData,
            }
        }

        /// Points at `instruction`'s own account metas and data. Sound because
        /// `StableAccountMeta` has `solana_instruction::AccountMeta`'s exact
        /// layout (checked in this module's tests), and the runtime reads only
        /// each vector's address and length.
        #[inline(always)]
        pub(crate) fn from_instruction(instruction: &'a solana_instruction::Instruction) -> Self {
            Self {
                accounts: StableVec {
                    addr: instruction.accounts.as_ptr() as u64,
                    cap: instruction.accounts.len() as u64,
                    len: instruction.accounts.len() as u64,
                    _marker: core::marker::PhantomData,
                },
                data: StableVec::borrowing(&instruction.data),
                program_id: instruction.program_id,
                _borrows: core::marker::PhantomData,
            }
        }
    }

    /// Invokes `instruction` through the `sol_invoke_signed_rust` syscall with
    /// no `RefCell` borrow checks: the caller must not pass a writable account
    /// whose data or lamports are borrowed elsewhere. Goes through
    /// `solana_define_syscall`, since on SBPF v3 a plain `extern "C"`
    /// declaration is not a syscall call.
    #[inline(always)]
    pub(crate) fn invoke_signed_raw(
        instruction: &StableInstruction<'_>,
        account_infos: &[AccountInfo<'_>],
        signers_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()> {
        #[cfg(target_os = "solana")]
        {
            let result = unsafe {
                solana_define_syscall::definitions::sol_invoke_signed_rust(
                    instruction as *const StableInstruction<'_> as *const u8,
                    account_infos.as_ptr() as *const u8,
                    account_infos.len() as u64,
                    signers_seeds.as_ptr() as *const u8,
                    signers_seeds.len() as u64,
                )
            };
            if result != 0 {
                return Err(result.into());
            }
        }
        #[cfg(not(target_os = "solana"))]
        let _ = (instruction, account_infos, signers_seeds);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use {
            super::*,
            alloc::vec,
            core::mem::{align_of, offset_of, size_of},
            solana_instruction::{AccountMeta, Instruction},
            solana_stable_layout::stable_instruction::StableInstruction as Official,
        };

        fn bytes_of<T>(value: &T) -> &[u8] {
            unsafe { core::slice::from_raw_parts(value as *const T as *const u8, size_of::<T>()) }
        }

        #[test]
        fn layouts_match_the_sdk() {
            assert_eq!(size_of::<StableInstruction<'_>>(), size_of::<Official>());
            assert_eq!(align_of::<StableInstruction<'_>>(), align_of::<Official>());
            assert_eq!(
                offset_of!(StableInstruction<'_>, accounts),
                offset_of!(Official, accounts)
            );
            assert_eq!(
                offset_of!(StableInstruction<'_>, data),
                offset_of!(Official, data)
            );
            assert_eq!(
                offset_of!(StableInstruction<'_>, program_id),
                offset_of!(Official, program_id)
            );

            assert_eq!(size_of::<StableAccountMeta>(), size_of::<AccountMeta>());
            assert_eq!(align_of::<StableAccountMeta>(), align_of::<AccountMeta>());
            assert_eq!(
                offset_of!(StableAccountMeta, pubkey),
                offset_of!(AccountMeta, pubkey)
            );
            assert_eq!(
                offset_of!(StableAccountMeta, is_signer),
                offset_of!(AccountMeta, is_signer)
            );
            assert_eq!(
                offset_of!(StableAccountMeta, is_writable),
                offset_of!(AccountMeta, is_writable)
            );
        }

        #[test]
        fn encoding_matches_the_sdk_for_the_same_instruction() {
            let program_id = solana_address::Address::new_from_array([7; 32]);
            let first = solana_address::Address::new_from_array([9; 32]);
            let second = solana_address::Address::new_from_array([5; 32]);
            let data = [1u8, 2, 3, 4, 5];

            let official = Official::from(Instruction {
                program_id,
                accounts: vec![
                    AccountMeta {
                        pubkey: first,
                        is_signer: true,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: second,
                        is_signer: false,
                        is_writable: true,
                    },
                ],
                data: data.to_vec(),
            });

            let accounts = [
                StableAccountMeta {
                    pubkey: first,
                    is_signer: true,
                    is_writable: false,
                },
                StableAccountMeta {
                    pubkey: second,
                    is_signer: false,
                    is_writable: true,
                },
            ];
            let ours = StableInstruction::new(program_id, &accounts, &data);

            assert_eq!(ours.accounts.len, official.accounts.len);
            assert_eq!(ours.data.len, official.data.len);
            assert_eq!(bytes_of(&ours.program_id), bytes_of(&official.program_id));

            let official_metas = unsafe {
                core::slice::from_raw_parts(
                    official.accounts.addr as *const AccountMeta,
                    official.accounts.len as usize,
                )
            };
            for (mine, theirs) in accounts.iter().zip(official_metas) {
                assert_eq!(bytes_of(mine), bytes_of(theirs));
            }
            let official_data = unsafe {
                core::slice::from_raw_parts(
                    official.data.addr as *const u8,
                    official.data.len as usize,
                )
            };
            assert_eq!(official_data, data);
        }

        #[test]
        fn from_instruction_encodes_like_the_sdk() {
            let instruction = Instruction {
                program_id: solana_address::Address::new_from_array([3; 32]),
                accounts: vec![
                    AccountMeta {
                        pubkey: solana_address::Address::new_from_array([4; 32]),
                        is_signer: true,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: solana_address::Address::new_from_array([6; 32]),
                        is_signer: false,
                        is_writable: false,
                    },
                ],
                data: vec![9, 8, 7],
            };
            let official = Official::from(instruction.clone());
            let ours = StableInstruction::from_instruction(&instruction);

            assert_eq!(ours.accounts.len, official.accounts.len);
            assert_eq!(ours.data.len, official.data.len);
            assert_eq!(bytes_of(&ours.program_id), bytes_of(&official.program_id));

            let meta_bytes = official.accounts.len as usize * size_of::<AccountMeta>();
            let our_metas =
                unsafe { core::slice::from_raw_parts(ours.accounts.addr as *const u8, meta_bytes) };
            let official_metas = unsafe {
                core::slice::from_raw_parts(official.accounts.addr as *const u8, meta_bytes)
            };
            assert_eq!(our_metas, official_metas);

            let our_data = unsafe {
                core::slice::from_raw_parts(ours.data.addr as *const u8, ours.data.len as usize)
            };
            let official_data = unsafe {
                core::slice::from_raw_parts(
                    official.data.addr as *const u8,
                    official.data.len as usize,
                )
            };
            assert_eq!(our_data, official_data);
        }
    }
}

/// The `solana_cpi` 3.1.0 API over the SDK's `AccountInfo`, with the same
/// signatures, the same pre-invoke borrow checks and the same syscalls, so a
/// program calling these behaves as it would with that crate.
#[cfg(not(feature = "pinocchio"))]
pub mod sdk {
    use {
        super::stable::{invoke_signed_raw, StableInstruction},
        crate::prelude::Vec,
        solana_account_info::AccountInfo,
        solana_address::Address,
        solana_instruction::Instruction,
        solana_program_error::ProgramResult,
    };

    /// [`invoke_signed`] with no signer seeds.
    pub fn invoke(instruction: &Instruction, account_infos: &[AccountInfo]) -> ProgramResult {
        invoke_signed(instruction, account_infos, &[])
    }

    /// [`invoke_signed_unchecked`] with no signer seeds.
    pub fn invoke_unchecked(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
    ) -> ProgramResult {
        invoke_signed_unchecked(instruction, account_infos, &[])
    }

    /// Invokes `instruction` after checking, for each of its account metas,
    /// the first entry of `account_infos` with that key: a writable meta's
    /// lamports and data must be mutably borrowable, a read-only meta's
    /// borrowable, otherwise this returns `AccountBorrowFailed` without
    /// invoking. The check borrows are released before the call.
    pub fn invoke_signed(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
        signers_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        for account_meta in instruction.accounts.iter() {
            for account_info in account_infos.iter() {
                if account_meta.pubkey == *account_info.key {
                    if account_meta.is_writable {
                        let _ = account_info.try_borrow_mut_lamports()?;
                        let _ = account_info.try_borrow_mut_data()?;
                    } else {
                        let _ = account_info.try_borrow_lamports()?;
                        let _ = account_info.try_borrow_data()?;
                    }
                    break;
                }
            }
        }

        invoke_signed_unchecked(instruction, account_infos, signers_seeds)
    }

    /// Invokes `instruction` with no borrow checks: the caller must not pass a
    /// writable account whose data or lamports are borrowed elsewhere.
    /// Off-chain this does nothing and returns `Ok(())`.
    pub fn invoke_signed_unchecked(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
        signers_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        invoke_signed_raw(
            &StableInstruction::from_instruction(instruction),
            account_infos,
            signers_seeds,
        )
    }

    /// The largest return data the runtime accepts.
    pub const MAX_RETURN_DATA: usize = 1024;

    /// Sets the current instruction's return data. Off-chain this does nothing.
    pub fn set_return_data(data: &[u8]) {
        #[cfg(target_os = "solana")]
        unsafe {
            solana_define_syscall::definitions::sol_set_return_data(
                data.as_ptr(),
                data.len() as u64,
            )
        };
        #[cfg(not(target_os = "solana"))]
        let _ = data;
    }

    /// The return data most recently set during this transaction and the id
    /// of the program that set it, at most [`MAX_RETURN_DATA`] bytes; `None`
    /// when it is empty. Off-chain always `None`.
    pub fn get_return_data() -> Option<(Address, Vec<u8>)> {
        #[cfg(target_os = "solana")]
        {
            let mut buf = [0u8; MAX_RETURN_DATA];
            let mut program_id = Address::default();

            let size = unsafe {
                solana_define_syscall::definitions::sol_get_return_data(
                    buf.as_mut_ptr(),
                    buf.len() as u64,
                    &mut program_id as *mut _ as *mut u8,
                )
            };

            if size == 0 {
                None
            } else {
                let size = core::cmp::min(size as usize, MAX_RETURN_DATA);
                Some((program_id, buf[..size].to_vec()))
            }
        }

        #[cfg(not(target_os = "solana"))]
        None
    }

    #[cfg(test)]
    mod tests {
        use {
            super::*, alloc::vec, solana_instruction::AccountMeta,
            solana_program_error::ProgramError,
        };

        fn key(byte: u8) -> Address {
            Address::new_from_array([byte; 32])
        }

        fn instruction(metas: &[(u8, bool)]) -> Instruction {
            Instruction {
                program_id: key(0xEE),
                accounts: metas
                    .iter()
                    .map(|&(byte, is_writable)| AccountMeta {
                        pubkey: key(byte),
                        is_signer: false,
                        is_writable,
                    })
                    .collect(),
                data: vec![1, 2, 3],
            }
        }

        fn both(instruction: &Instruction, infos: &[AccountInfo]) -> ProgramResult {
            let ours = invoke_signed(instruction, infos, &[]);
            let stock = solana_cpi::invoke_signed(instruction, infos, &[]);
            assert!(ours == stock);
            ours
        }

        #[test]
        fn borrow_checks_match_solana_cpi() {
            let (k1, k2, owner) = (key(1), key(2), key(0xAA));
            let (mut lamports1, mut lamports2) = (10u64, 20u64);
            let (mut data1, mut data2) = ([0u8; 4], [0u8; 4]);
            let first =
                AccountInfo::new(&k1, false, true, &mut lamports1, &mut data1, &owner, false);
            let second =
                AccountInfo::new(&k2, false, false, &mut lamports2, &mut data2, &owner, false);
            let infos = [first.clone(), second.clone()];

            assert!(both(&instruction(&[(1, true), (2, false)]), &infos) == Ok(()));
            assert!(both(&instruction(&[(9, true)]), &infos) == Ok(()));

            {
                let _shared = first.data.borrow();
                assert!(
                    both(&instruction(&[(1, true)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
                assert!(both(&instruction(&[(1, false)]), &infos) == Ok(()));
            }
            {
                let _exclusive = first.lamports.borrow_mut();
                assert!(
                    both(&instruction(&[(1, false)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
            }
            {
                let _exclusive = second.data.borrow_mut();
                assert!(both(&instruction(&[(1, true)]), &infos) == Ok(()));
                assert!(
                    both(&instruction(&[(1, true), (2, false)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
            }
        }

        #[test]
        fn only_the_first_matching_account_info_is_checked() {
            let (k1, owner) = (key(1), key(0xAA));
            let (mut lamports_a, mut lamports_b) = (1u64, 2u64);
            let (mut data_a, mut data_b) = ([0u8; 2], [0u8; 2]);
            let free = AccountInfo::new(
                &k1,
                false,
                true,
                &mut lamports_a,
                &mut data_a,
                &owner,
                false,
            );
            let busy = AccountInfo::new(
                &k1,
                false,
                true,
                &mut lamports_b,
                &mut data_b,
                &owner,
                false,
            );
            let _held = busy.data.borrow_mut();

            assert!(both(&instruction(&[(1, true)]), &[free.clone(), busy.clone()]) == Ok(()));
            assert!(
                both(&instruction(&[(1, true)]), &[busy.clone(), free.clone()])
                    == Err(ProgramError::AccountBorrowFailed)
            );
        }
    }
}

// ===========================================================================
// PINOCCHIO BACKEND
// ===========================================================================

/// Bridges naclac's own `CpiHandle` to the real
/// `solana_instruction_view::cpi` (re-exported as `pinocchio::cpi`) checked
/// invoke helpers, which are generic over `AsRef<AccountView>`.
#[cfg(feature = "pinocchio")]
impl<'a> AsRef<::pinocchio::AccountView> for crate::prelude::CpiHandle<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &::pinocchio::AccountView {
        &self.info.view
    }
}

#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_handles,
        CpiSigner::as_pinocchio(signers),
    )
}

/// Borrow-checked CPI for an instruction whose account metas were built from
/// the same views, in the same order, as `account_handles`: the generated CPI
/// clients and naclac's own instruction builders. Every writable account must
/// not be borrowed, as in [`invoke_signed_pinocchio_handles`], but the
/// per-account address comparison is left out, because that comparison only
/// pairs a view with its meta so the borrow check looks at the right
/// account, and the pairing holds by construction here. A caller that cannot
/// guarantee it must use [`invoke_signed_pinocchio_handles`].
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_paired(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    let accounts_len = instruction.accounts.len();
    if accounts_len > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }
    if account_handles.len() < accounts_len {
        return Err(crate::prelude::NaclacError::NotEnoughAccountKeys.into());
    }

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() };
            crate::prelude::MAX_CPI_ACCOUNTS];
    for ((slot, handle), meta) in cpi_accounts_arr
        .iter_mut()
        .zip(account_handles.iter())
        .zip(instruction.accounts.iter())
    {
        debug_assert!(handle.info.view.address() == meta.address);
        if meta.is_writable {
            handle.info.view.check_borrow_mut()?;
        }
        ::pinocchio::cpi::CpiAccount::init_from_account_view(&handle.info.view, slot);
    }
    // SAFETY: the loop above initialised the first `accounts_len` slots, since
    // both `account_handles` and `instruction.accounts` have at least that
    // many entries.
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            accounts_len,
        )
    };

    // SAFETY: every writable account was checked to be unborrowed above.
    unsafe {
        ::pinocchio::cpi::invoke_signed_unchecked(
            instruction,
            cpi_accounts,
            CpiSigner::as_pinocchio(signers),
        );
    }
    Ok(())
}

/// [`invoke_signed_pinocchio_paired`] for a call site whose account count is
/// known at compile time: `accounts[i]` must be the account of `handles[i]`,
/// and the fixed length lets the per-account loop unroll.
#[cfg(feature = "pinocchio")]
#[inline(always)]
pub fn invoke_signed_pinocchio_paired_array<const N: usize>(
    program: &crate::prelude::CpiHandle<'_>,
    accounts: &[::pinocchio::instruction::InstructionAccount<'_>; N],
    handles: &[crate::prelude::CpiHandle<'_>; N],
    data: &[u8],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    const { assert!(N <= crate::prelude::MAX_CPI_ACCOUNTS) };

    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts,
        data,
    };

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() }; N];
    let mut i = 0;
    while i < N {
        debug_assert!(handles[i].info.view.address() == accounts[i].address);
        if accounts[i].is_writable {
            handles[i].info.view.check_borrow_mut()?;
        }
        ::pinocchio::cpi::CpiAccount::init_from_account_view(
            &handles[i].info.view,
            &mut cpi_accounts_arr[i],
        );
        i += 1;
    }
    // SAFETY: the loop above initialised all `N` slots.
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            N,
        )
    };

    // SAFETY: every writable account was checked to be unborrowed above.
    unsafe {
        ::pinocchio::cpi::invoke_signed_unchecked(
            &instruction,
            cpi_accounts,
            CpiSigner::as_pinocchio(signers),
        );
    }
    Ok(())
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_pinocchio`/
/// `invoke_signed_pinocchio_handles`.
#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles_unchecked(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    let account_infos: &[crate::prelude::AccountInfo] =
        unsafe { core::mem::transmute(account_handles) };

    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() };
            crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, info) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(&info.view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    unsafe {
        ::pinocchio::cpi::invoke_signed_unchecked(
            instruction,
            cpi_accounts,
            CpiSigner::as_pinocchio(signers),
        );
    }
    Ok(())
}

/// Checked CPI from a raw slice of `AccountView`s (rather than naclac's own
/// `CpiHandle`) — for hand-authored instruction bodies constructing an
/// `InstructionView` directly.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_infos,
        CpiSigner::as_pinocchio(signers),
    )
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signers: &[CpiSigner<'_, '_>],
) -> crate::prelude::Result<()> {
    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() };
            crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, view) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    unsafe {
        ::pinocchio::cpi::invoke_signed_unchecked(
            instruction,
            cpi_accounts,
            CpiSigner::as_pinocchio(signers),
        );
    }
    Ok(())
}
