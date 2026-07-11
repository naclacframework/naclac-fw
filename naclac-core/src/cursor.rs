//! # Lazy Account Cursor
//!
//! [`AccountCursor`] yields [`AccountView`]s on demand so dispatch can
//! happen first and each arm walks only its declared accounts. A
//! lookup array resolves duplicate account references: when the BPF
//! loader writes a dup index into `borrow_state`, the cursor returns
//! the earlier `AccountView` from `lookup[idx]`.
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
    pub unsafe fn new(input_ptr: *mut u8, lookup: *mut AccountView) -> Self {
        Self {
            // Skip the 8-byte num_accounts prefix — accounts start right after.
            ptr: input_ptr.add(size_of::<u64>()),
            lookup,
            consumed: 0,
            duplicate: None,
        }
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
        let account: *mut RuntimeAccount = self.ptr as *mut RuntimeAccount;

        // Advance 8 bytes at the head of every slot. Covers either:
        //  - non-dup: the first 8 bytes of the RuntimeAccount struct
        //    (borrow_state + signer + writable + executable + 4-byte padding)
        //  - dup: the entire dup slot (1-byte dup index + 7-byte padding)
        self.ptr = self.ptr.add(size_of::<u64>());

        // The first account (consumed == 0) can never be a duplicate.
        // Short-circuit the dup check to save a branch on the hot path.
        let borrow_state = (*account).borrow_state;
        let view = if self.consumed == 0 || borrow_state == NON_DUP_MARKER {
            // Non-dup: store the original data_len in the padding slot so
            // `AccountView::resize()` can enforce MAX_PERMITTED_DATA_INCREASE.
            (*account).padding = u32::to_le_bytes((*account).data_len as u32);
            let data_len = (*account).data_len as usize;

            // Advance past: static header region + variable account data.
            self.ptr = self.ptr.add(STATIC_ACCOUNT_DATA);
            self.ptr = self.ptr.add(data_len);

            // Align to the next 8-byte boundary.
            let addr = self.ptr.addr();
            let aligned = (addr + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
            self.ptr = self.ptr.add(aligned - addr);

            AccountView::new_unchecked(account)
        } else {
            // Duplicate: look up the earlier slot. Safe because the runtime
            // only emits dup indices strictly less than the current `consumed`,
            // so that slot is already populated by a prior `next()` call.
            // Lazily materialize the bitvec on first dup — non-dup txs pay zero.
            let bv = self.duplicate.get_or_insert_with(AccountBitvec::default);
            bv.set(self.consumed);
            bv.set(borrow_state);
            *self.lookup.add(borrow_state as usize)
        };

        // Record this view so later dup references can resolve it.
        *self.lookup.add(self.consumed as usize) = view;
        self.consumed = self.consumed.wrapping_add(1);
        view
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
/// This is used by the dispatcher to locate `ix_data` (and thus the 8-byte
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
