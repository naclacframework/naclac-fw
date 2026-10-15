//! # Lazy Account Cursor
//!
//! [`AccountCursor`] yields [`AccountView`]s on demand so dispatch can
//! happen first: each instruction's pinocchio `load_and_validate` walks its
//! declared accounts through [`AccountCursor::try_declared_fast`] (one header
//! compare per account on the common path), and
//! [`AccountCursor::finish_walk_slow`] takes over on the first miss or when
//! trailing accounts follow. A lookup array resolves duplicate account
//! references: when the BPF loader writes a dup index into `borrow_state`,
//! the cursor returns the earlier `AccountView` from `lookup[idx]`.
//!
//! This is a port of Anchor lang-v2's `cursor.rs`, adapted for naclac.
//!
//! ## Input buffer layout (from pinocchio entrypoint mod.rs)
//!
//! ```text
//! ┌─ 8 bytes (u64): number of accounts
//! ├─ For each account:
//! │   ├─ 1 byte: dup marker (0xFF = not dup, else = dup index)
//! │   ├─ If dup: 7 bytes padding (total 8 bytes per dup slot)
//! │   └─ If not dup:
//! │         ├─ 7 bytes: signer/writable/executable + 4 bytes padding
//! │         ├─ 32 bytes: address
//! │         ├─ 32 bytes: owner
//! │         ├─ 8 bytes: lamports (u64)
//! │         ├─ 8 bytes: data_len (u64)
//! │         ├─ data_len bytes: account data
//! │         ├─ MAX_PERMITTED_DATA_INCREASE bytes: resize buffer (10240)
//! │         ├─ alignment to 8 bytes
//! │         └─ 8 bytes: rent_epoch (u64)
//! ├─ 8 bytes (u64): instruction data length
//! ├─ instruction data bytes
//! └─ 32 bytes: program_id (Address)
//! ```

#[cfg(feature = "pinocchio")]
use core::mem::size_of;
#[cfg(feature = "pinocchio")]
use pinocchio::account::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE};

/// Sentinel value indicating a non-duplicated account.
/// Any other value (0..=254) is the index of the earlier account it aliases.
pub const NON_DUP_MARKER: u8 = u8::MAX;

#[cfg(feature = "pinocchio")]
const STATIC_ACCOUNT_DATA: usize = size_of::<RuntimeAccount>() + MAX_PERMITTED_DATA_INCREASE;

#[cfg(feature = "pinocchio")]
const BPF_ALIGN_OF_U128: usize = 8;

/// The start of the record after the non-duplicate account record `account`
/// that begins at `ptr`.
///
/// # Safety
///
/// `ptr` must point at the non-duplicate record `account` in the entrypoint
/// input buffer.
#[cfg(feature = "pinocchio")]
#[inline(always)]
unsafe fn next_record(ptr: *mut u8, account: *mut RuntimeAccount) -> *mut u8 {
    // The account's `data_len` bytes physically follow its header in the
    // input buffer, so the record size cannot overflow `usize`.
    let data_len = (*account).data_len as usize;
    let ptr = ptr.add((size_of::<u64>() + STATIC_ACCOUNT_DATA).wrapping_add(data_len));
    ptr.add(ptr.addr().wrapping_neg() & (BPF_ALIGN_OF_U128 - 1))
}

/// Cursor into the serialized instruction input buffer.
///
/// Advances a raw pointer past one account record per [`next`](AccountCursor::next)
/// call and uses a `lookup` array for dup resolution.
///
/// # Safety
///
/// Created from the runtime's `input` pointer (the single arg to
/// `extern "C" fn entrypoint(input: *mut u8)`). Must not outlive the
/// entrypoint invocation. Callers must ensure:
///
/// - `lookup` points to `[AccountView; N]` where `N >= max(consumed + 1,
///   max_dup_index + 1)`. In practice: a `[MaybeUninit<AccountView>; 64]`
///   allocated in the dispatcher frame and passed as `*mut AccountView`.
/// - `next()` is called fewer than `num_accounts` times.
#[cfg(feature = "pinocchio")]
pub struct AccountCursor {
    /// Current position in the input buffer. Advances on each `next()`.
    ptr: *mut u8,

    /// Pointer to the caller's `[AccountView; N]` lookup array.
    /// Indexed by `consumed` on write and by the serialized dup index on read.
    lookup: *mut AccountView,

    /// Number of accounts yielded so far. Used both as the write index
    /// into `lookup` and as a runtime counter for bookkeeping.
    consumed: u8,

    /// Tracks accounts that are duplicates — lazily initialized on first dup.
    /// `None` for transactions with no duplicates (the common case).
    duplicate: Option<AccountBitvec>,

    /// The executing program's id. An optional account slot that is absent
    /// holds this address, so duplicates resolving to it are not aliases of a
    /// real account and are never recorded in `duplicate`.
    program_id: &'static crate::prelude::Address,
}

/// The little-endian `u32` at the start of every non-duplicate serialized
/// account: `[dup marker, is_signer, is_writable, executable]`.
#[cfg(feature = "pinocchio")]
const HEADER_NOT_DUP: u32 = NON_DUP_MARKER as u32;
#[cfg(feature = "pinocchio")]
const HEADER_SIGNER: u32 = 1 << 8;
#[cfg(feature = "pinocchio")]
const HEADER_WRITABLE: u32 = 1 << 16;
#[cfg(feature = "pinocchio")]
const HEADER_EXECUTABLE: u32 = 1 << 24;

#[cfg(test)]
mod account_header_check_tests {
    use super::account_header_check;

    /// Builds the header word the runtime serializes for a non-duplicate
    /// account: `[0xFF, is_signer, is_writable, executable]`, little-endian.
    fn header(signer: bool, writable: bool, executable: bool) -> u32 {
        u32::from_le_bytes([0xFF, signer as u8, writable as u8, executable as u8])
    }

    #[test]
    fn accepts_exactly_the_headers_that_carry_the_required_flags() {
        for req in 0..8u8 {
            let (rs, rw, rx) = (req & 1 != 0, req & 2 != 0, req & 4 != 0);
            let (expected, mask) = account_header_check(rs, rw, rx);
            for have in 0..8u8 {
                let (hs, hw, hx) = (have & 1 != 0, have & 2 != 0, have & 4 != 0);
                let passes = header(hs, hw, hx) & mask == expected;
                let satisfies = (!rs || hs) && (!rw || hw) && (!rx || hx);
                assert!(passes == satisfies);
            }
        }
    }

    #[test]
    fn rejects_every_duplicate_marker() {
        let (expected, mask) = account_header_check(false, false, false);
        for marker in 0..u8::MAX {
            let dup = u32::from_le_bytes([marker, 1, 1, 1]);
            assert!(dup & mask != expected);
        }
    }
}

/// The `(expected, mask)` pair for [`AccountCursor::try_declared_fast`]: an
/// account passes the fast path when `header & mask == expected`, i.e. it is
/// not a duplicate and carries at least the required flags.
pub const fn account_header_check(signer: bool, writable: bool, executable: bool) -> (u32, u32) {
    let mut expected = NON_DUP_MARKER as u32;
    let mut mask = 0xFF;
    if signer {
        expected |= 1 << 8;
        mask |= 0xFF << 8;
    }
    if writable {
        expected |= 1 << 16;
        mask |= 0xFF << 16;
    }
    if executable {
        expected |= 1 << 24;
        mask |= 0xFF << 24;
    }
    (expected, mask)
}

#[cfg(feature = "pinocchio")]
impl AccountCursor {
    /// Create a fresh cursor at the start of the serialized accounts region.
    /// `input_ptr` must point at the 8-byte `num_accounts` length prefix
    /// (i.e. the runtime-provided `input` argument to entrypoint); the cursor
    /// advances past it internally.
    ///
    /// # Safety
    ///
    /// See type-level safety notes. `lookup` must be a valid pointer to at
    /// least `num_accounts` contiguous `AccountView` slots.
    #[inline(always)]
    pub unsafe fn new(
        input_ptr: *mut u8,
        lookup: *mut AccountView,
        program_id: &'static crate::prelude::Address,
    ) -> Self {
        Self {
            // Skip the 8-byte num_accounts prefix — accounts start right after.
            ptr: input_ptr.add(size_of::<u64>()),
            lookup,
            consumed: 0,
            duplicate: None,
            program_id,
        }
    }

    /// Advances past the non-duplicate account record at `self.ptr` and
    /// stores its view at `lookup[consumed]`.
    ///
    /// # Safety
    ///
    /// `self.ptr` must point at a non-duplicate account record.
    #[inline(always)]
    unsafe fn take_non_dup(&mut self) -> AccountView {
        let slot = self.consumed as usize;
        let view = self.advance_non_dup();
        *self.lookup.add(slot) = view;
        view
    }

    /// Advances past the non-duplicate account record at `self.ptr` without
    /// storing its view in `lookup`.
    ///
    /// # Safety
    ///
    /// `self.ptr` must point at a non-duplicate account record.
    #[inline(always)]
    unsafe fn advance_non_dup(&mut self) -> AccountView {
        let account: *mut RuntimeAccount = self.ptr as *mut RuntimeAccount;
        // Store the original data_len in the padding slot so
        // `AccountView::resize()` can enforce MAX_PERMITTED_DATA_INCREASE.
        (*account).padding = u32::to_le_bytes((*account).data_len as u32);
        self.ptr = next_record(self.ptr, account);
        self.consumed = self.consumed.wrapping_add(1);
        AccountView::new_unchecked(account)
    }

    /// Stores into `lookup[..consumed]` the views of the accounts already
    /// walked by [`try_declared_fast`](AccountCursor::try_declared_fast),
    /// which does not record them, by re-walking the records from `input`.
    ///
    /// # Safety
    ///
    /// `input` must be the entrypoint input buffer this cursor was created
    /// from, and every slot before `consumed` must have been accepted by
    /// `try_declared_fast` (so each is a non-duplicate record).
    #[inline(always)]
    unsafe fn record_fast_walked(&mut self, input: *mut u8) {
        let mut ptr = input.add(size_of::<u64>());
        for slot in 0..self.consumed as usize {
            let account = ptr as *mut RuntimeAccount;
            *self.lookup.add(slot) = AccountView::new_unchecked(account);
            ptr = next_record(ptr, account);
        }
    }

    /// Resolves the duplicate record at `self.ptr`, whose marker is `orig`,
    /// to the earlier view it aliases, and records the pair in `duplicate`
    /// unless that view is the program's own address (an absent optional
    /// slot's sentinel).
    ///
    /// # Safety
    ///
    /// `self.ptr` must point at a duplicate record and `orig < consumed`.
    #[cold]
    #[inline(never)]
    unsafe fn take_dup(&mut self, orig: u8) -> AccountView {
        let view = *self.lookup.add(orig as usize);
        if crate::prelude::Address::from_address_ref(view.address()) != self.program_id {
            let bv = self.duplicate.get_or_insert_with(AccountBitvec::default);
            bv.set(self.consumed);
            bv.set(orig);
        }
        self.ptr = self.ptr.add(size_of::<u64>());
        *self.lookup.add(self.consumed as usize) = view;
        self.consumed = self.consumed.wrapping_add(1);
        view
    }

    /// Fast path for the next declared field, whose required flags are
    /// encoded in `(expected, mask)` (see [`account_header_check`]): when the
    /// account is a non-duplicate carrying at least those flags, consumes it
    /// with a single compare and returns its view, without storing it in
    /// `lookup`. Otherwise consumes nothing and returns `None`; the caller
    /// must then finish the walk with
    /// [`finish_walk_slow`](AccountCursor::finish_walk_slow), which records
    /// the views this method skipped.
    ///
    /// # Safety
    ///
    /// Must not be called once `consumed` has reached the instruction's
    /// account count.
    #[inline(always)]
    pub unsafe fn try_declared_fast(&mut self, expected: u32, mask: u32) -> Option<AccountView> {
        let header = *(self.ptr as *const u32);
        if header & mask == expected {
            return Some(self.advance_non_dup());
        }
        None
    }

    /// Completes the account walk after the
    /// [`try_declared_fast`](AccountCursor::try_declared_fast) chain either
    /// missed or left trailing accounts:
    ///
    /// 0. Records into `lookup` the views of the slots the fast chain already
    ///    walked (re-walked from `input`), so `lookup[..num_accounts]` holds
    ///    every account's view when this returns `Ok`.
    /// 1. Walks every slot from `consumed` to `num_accounts`, resolving
    ///    duplicates (`AccountBorrowFailed` for that slot's index if a
    ///    duplicate marker does not point at an earlier slot). A slot below
    ///    `checks.len()` is a declared field whose `checks[slot]` is
    ///    `(expected, mask, optional)`: an absent `optional` slot (address
    ///    equal to the program id) is accepted without any flag requirement,
    ///    and otherwise `ConstraintSigner`, `ConstraintMut` or
    ///    `ConstraintExecutable` (in that order of precedence) is returned for
    ///    the slot's index when a required flag is missing.
    /// 2. Returns `ConstraintDuplicateMutableAccount` (index 0) if any
    ///    duplicate recorded during the walk involves a slot set in
    ///    `mut_mask`.
    ///
    /// This is the only place duplicates can be recorded: the fast path only
    /// accepts non-duplicate headers, so a walk that never reaches this
    /// function has no duplicates to check. Takes the cursor by value so the
    /// caller's cursor never has its address taken and can stay in registers.
    ///
    /// # Safety
    ///
    /// `input` must be the entrypoint input buffer this cursor was created
    /// from; `num_accounts` must be the instruction's account count, at least
    /// `checks.len()` and at most the `lookup` capacity; and every slot before
    /// `consumed` must have been accepted by `try_declared_fast` as a declared
    /// field of the same `checks`.
    #[cold]
    #[inline(never)]
    pub unsafe fn finish_walk_slow(
        mut self,
        input: *mut u8,
        checks: &[(u32, u32, bool)],
        num_accounts: usize,
        mut_mask: &[u64; 4],
    ) -> Result<(), crate::prelude::ProgramError> {
        self.record_fast_walked(input);
        while (self.consumed as usize) < num_accounts {
            let slot = self.consumed as usize;
            let marker = *self.ptr;
            let view = if marker == NON_DUP_MARKER {
                self.take_non_dup()
            } else {
                if marker >= self.consumed {
                    return Err(crate::error::NaclacError::AccountBorrowFailed.err(slot));
                }
                self.take_dup(marker)
            };
            if let Some(&(expected, mask, optional)) = checks.get(slot) {
                self.check_declared_flags(&view, slot, expected, mask, optional)?;
            }
        }
        if let Some(duplicates) = &self.duplicate {
            if duplicates.intersects(mut_mask) {
                return Err(crate::error::NaclacError::ConstraintDuplicateMutableAccount.err(0));
            }
        }
        Ok(())
    }

    /// Checks a declared field's account `view` at index `slot` against its
    /// `(expected, mask, optional)` requirement; see
    /// [`finish_walk_slow`](AccountCursor::finish_walk_slow).
    #[inline(always)]
    fn check_declared_flags(
        &self,
        view: &AccountView,
        slot: usize,
        expected: u32,
        mask: u32,
        optional: bool,
    ) -> Result<(), crate::prelude::ProgramError> {
        if optional && crate::prelude::Address::from_address_ref(view.address()) == self.program_id
        {
            return Ok(());
        }
        let mut actual = HEADER_NOT_DUP;
        if view.is_signer() {
            actual |= HEADER_SIGNER;
        }
        if view.is_writable() {
            actual |= HEADER_WRITABLE;
        }
        if view.executable() {
            actual |= HEADER_EXECUTABLE;
        }
        let missing = expected & !(actual & mask);
        if missing & HEADER_SIGNER != 0 {
            return Err(crate::error::NaclacError::ConstraintSigner.err(slot));
        }
        if missing & HEADER_WRITABLE != 0 {
            return Err(crate::error::NaclacError::ConstraintMut.err(slot));
        }
        if missing & HEADER_EXECUTABLE != 0 {
            return Err(crate::error::NaclacError::ConstraintExecutable.err(slot));
        }
        Ok(())
    }

    /// Number of accounts yielded from this cursor so far.
    #[inline(always)]
    pub fn consumed(&self) -> u8 {
        self.consumed
    }

    /// Current duplicate-tracking bitvec, if any duplicates have been seen.
    #[inline(always)]
    pub fn duplicates(&self) -> Option<&AccountBitvec> {
        self.duplicate.as_ref()
    }

    /// Walk N accounts in a tight loop, storing views in the lookup array.
    /// Returns a slice of the walked views.
    ///
    /// This runs all the pointer math first, before any validation logic,
    /// so LLVM can vectorize/optimize the walk loop.
    ///
    /// # Safety
    ///
    /// Caller must ensure `n` does not exceed the remaining unread accounts.
    #[inline(always)]
    pub unsafe fn walk_n(&mut self, n: usize) -> (&[AccountView], Option<&AccountBitvec>) {
        let start = self.consumed as usize;
        for _ in 0..n {
            self.next();
        }
        (
            core::slice::from_raw_parts(self.lookup.add(start), n),
            self.duplicate.as_ref(),
        )
    }

    /// Advance past one account record and return its [`AccountView`].
    ///
    /// Handles both non-duplicated accounts (walks past the record header +
    /// data + padding) and duplicated accounts (reads the earlier view from
    /// `lookup`). Also writes the resolved view back into `lookup[consumed]`
    /// so future dup references resolve correctly.
    ///
    /// # Safety
    ///
    /// Must not be called if `consumed` has already reached the transaction's
    /// total `num_accounts`. The caller (dispatcher or remaining_accounts walk)
    /// is responsible for bounds-checking upfront.
    #[inline(always)]
    pub unsafe fn next(&mut self) -> AccountView {
        // The first account (consumed == 0) can never be a duplicate; the
        // runtime only emits dup indices strictly less than `consumed`, so the
        // referenced slot is always already populated.
        let borrow_state = *self.ptr;
        if self.consumed == 0 || borrow_state == NON_DUP_MARKER {
            self.take_non_dup()
        } else {
            self.take_dup(borrow_state)
        }
    }
}

// SAFETY: The cursor contains raw pointers into the BPF input buffer.
// Solana's single-threaded SBF execution model means there is no concurrent
// access; Send/Sync are safe within the transaction lifetime.
#[cfg(feature = "pinocchio")]
unsafe impl Send for AccountCursor {}
#[cfg(feature = "pinocchio")]
unsafe impl Sync for AccountCursor {}

// ─────────────────────────────────────────────────────────────────────────────
// Input buffer navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Walk past all `num_accounts` account records in the input buffer without
/// building `AccountView`s, returning a pointer to the first byte of the
/// instruction-data section.
///
/// This is used by the dispatcher to locate `ix_data` (and thus the
/// discriminator) **before** constructing the per-arm `AccountCursor`.
///
/// # Safety
///
/// `input` must be the raw BPF entrypoint input buffer (as provided by the
/// loader). `num_accounts` must match the value at `input[0..8]`.
#[cfg(feature = "pinocchio")]
#[inline(always)]
pub unsafe fn skip_accounts(input: *mut u8, num_accounts: usize) -> *mut u8 {
    // Skip the 8-byte num_accounts prefix.
    let mut ptr = input.add(size_of::<u64>());

    for i in 0..num_accounts {
        let account: *mut RuntimeAccount = ptr as *mut RuntimeAccount;
        // Always advance 8 bytes (covers the first 8 bytes of every slot).
        ptr = ptr.add(size_of::<u64>());

        // First slot can never be dup; for subsequent slots check the marker.
        let borrow_state = (*account).borrow_state;
        if i == 0 || borrow_state == NON_DUP_MARKER {
            // Non-dup: advance past static header + variable data + alignment.
            let data_len = (*account).data_len as usize;
            ptr = ptr.add(STATIC_ACCOUNT_DATA);
            ptr = ptr.add(data_len);
            // Align to 8 bytes.
            let addr = ptr.addr();
            let aligned = (addr + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
            ptr = ptr.add(aligned - addr);
        }
        // Dup slot: the 8 bytes already advanced above is the entire record.
    }

    ptr
}

/// Parse the instruction context from the raw BPF input buffer.
///
/// Returns `(program_id, ix_data)` by locating them after the accounts section.
/// This is called once at entrypoint entry to get the discriminator, before
/// the per-arm cursor is constructed.
///
/// # Safety
///
/// `input` must be the valid BPF entrypoint buffer. `num_accounts` must equal
/// `*(input as *const u64)`.
#[cfg(feature = "pinocchio")]
#[inline(always)]
pub unsafe fn parse_ix_context(
    input: *mut u8,
    num_accounts: usize,
) -> Result<(&'static pinocchio::Address, &'static [u8]), pinocchio::error::ProgramError> {
    // Walk past all accounts to find the ix_data section.
    let mut ptr = skip_accounts(input, num_accounts);

    // Read ix_data_len (8 bytes, little-endian u64).
    let ix_data_len = *(ptr as *const u64) as usize;
    ptr = ptr.add(size_of::<u64>());

    // Slice the instruction data.
    let ix_data: &'static [u8] = core::slice::from_raw_parts(ptr, ix_data_len);
    ptr = ptr.add(ix_data_len);

    // Read the program_id (32 bytes, at the very end of the buffer).
    let program_id: &'static pinocchio::Address = &*(ptr as *const pinocchio::Address);

    Ok((program_id, ix_data))
}

/// The error for an instruction whose account count is outside
/// `declared..=64`: `TooManyAccountEntries` above 64 (the lookup capacity),
/// otherwise `NotEnoughAccountKeys`.
#[cfg(feature = "pinocchio")]
#[cold]
#[inline(never)]
pub fn account_count_error(num_accounts: usize) -> pinocchio::error::ProgramError {
    if num_accounts > 64 {
        crate::error::NaclacError::TooManyAccountEntries.err(0)
    } else {
        crate::error::NaclacError::NotEnoughAccountKeys.err(0)
    }
}

// Helper for building MUT_MASK at compile time
pub const fn mut_mask_set_bit(mut mask: [u64; 4], bit: usize) -> [u64; 4] {
    mask[bit / 64] |= 1u64 << (bit % 64);
    mask
}

// ─────────────────────────────────────────────────────────────────────────────
// AccountBitvec — 256-bit bitvec for duplicate tracking
// ─────────────────────────────────────────────────────────────────────────────

/// A 256-bit bitvec used to track duplicate and mutable accounts.
///
/// Does not derive `Copy` to avoid accidental large stack moves.
#[derive(Default, Clone)]
pub struct AccountBitvec {
    data: [u64; 4],
}

impl AccountBitvec {
    #[inline]
    pub fn get(&self, index: u8) -> bool {
        let index = index as usize;
        let arr_index = index / 64;
        let bit_index = index % 64;
        (self.data[arr_index] >> bit_index) & 1 == 1
    }

    #[inline]
    pub fn set(&mut self, index: u8) {
        let index = index as usize;
        let arr_index = index / 64;
        let bit_index = index % 64;
        self.data[arr_index] |= 1 << bit_index;
    }

    /// Returns `true` iff any bit set in `self` is also set in `mask`.
    /// Used to check if a trailing account's dup index resolves to a
    /// declared mutable slot — catches aliasing across remaining_accounts.
    #[inline]
    pub fn intersects(&self, mask: &[u64; 4]) -> bool {
        (self.data[0] & mask[0])
            | (self.data[1] & mask[1])
            | (self.data[2] & mask[2])
            | (self.data[3] & mask[3])
            != 0
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `get`/`set`/`intersects` never panic for any `u8` index —
    /// `arr_index = index / 64` must stay `< 4` for the full `0..=255` range,
    /// which this exhaustively checks rather than assumes from the type
    /// shapes lining up.
    #[kani::proof]
    fn prove_account_bitvec_get_set_never_panics() {
        let mut bv = AccountBitvec::default();
        let index: u8 = kani::any();
        bv.set(index);
        let _ = bv.get(index);
        let mask: [u64; 4] = kani::any();
        let _ = bv.intersects(&mask);
    }

    /// Correctness, not just panic-freedom: setting a bit must make `get`
    /// report it set. A `set` that missed the real underlying bit (a wrong
    /// shift/index derivation) would silently defeat the whole
    /// duplicate-mutable-account guard while still "not panicking".
    #[kani::proof]
    fn prove_account_bitvec_set_then_get_is_true() {
        let mut bv = AccountBitvec::default();
        let index: u8 = kani::any();
        bv.set(index);
        assert!(bv.get(index), "set(index) must make get(index) true");
    }

    /// Correctness: setting one bit must not disturb any other bit — a
    /// guard whose `set` had any bit-index collision would let two
    /// different accounts alias the same tracked bit, silently defeating
    /// duplicate detection for one of them.
    #[kani::proof]
    fn prove_account_bitvec_set_does_not_disturb_other_bits() {
        let mut bv = AccountBitvec::default();
        let set_index: u8 = kani::any();
        let other_index: u8 = kani::any();
        kani::assume(set_index != other_index);
        let before = bv.get(other_index);
        bv.set(set_index);
        let after = bv.get(other_index);
        assert_eq!(
            before, after,
            "set(set_index) must not change get(other_index) for other_index != set_index"
        );
    }

    /// Correctness for `intersects`: a bitvec with exactly one bit set must
    /// report an intersection against a mask with that same bit set, and no
    /// intersection against a mask with only a different bit set.
    #[kani::proof]
    fn prove_account_bitvec_intersects_correctness() {
        let mut bv = AccountBitvec::default();
        let bit: u8 = kani::any();
        bv.set(bit);

        let mut same_mask = [0u64; 4];
        same_mask[(bit as usize) / 64] |= 1u64 << ((bit as usize) % 64);
        assert!(
            bv.intersects(&same_mask),
            "matching bit must be detected as intersecting"
        );

        let other_bit: u8 = kani::any();
        kani::assume(other_bit != bit);
        let mut other_mask = [0u64; 4];
        other_mask[(other_bit as usize) / 64] |= 1u64 << ((other_bit as usize) % 64);
        assert!(
            !bv.intersects(&other_mask),
            "non-matching bit must not be reported as intersecting"
        );
    }

    /// `mut_mask_set_bit` is only ever called at macro-expansion time with a
    /// small literal `bit` (an account field's declaration index), never a
    /// user-controlled runtime value — its real contract is "safe for `bit
    /// < 256`", not "safe for all `usize`" (unlike `AccountBitvec::set`,
    /// whose `index: u8` bounds this at the type level, `bit` here is a bare
    /// `usize` with no such guarantee, and `mask[bit / 64]` panics
    /// out-of-bounds for `bit >= 256`). This proves the real contract
    /// exactly, plus the same set-bit/no-disturbance correctness
    /// `AccountBitvec::set` gets above — it does not claim totality over
    /// every `usize`, since that claim is false.
    #[kani::proof]
    fn prove_mut_mask_set_bit_within_contract() {
        let mask: [u64; 4] = kani::any();
        let bit: usize = kani::any();
        kani::assume(bit < 256);
        let result = mut_mask_set_bit(mask, bit);
        assert_eq!(
            (result[bit / 64] >> (bit % 64)) & 1,
            1,
            "the target bit must end up set"
        );
        for word in 0..4 {
            if word != bit / 64 {
                assert_eq!(
                    result[word], mask[word],
                    "words other than the target must be untouched"
                );
            }
        }
    }
}
