//! # Event Emission Routing
//!
//! Exposes a unified `emit_event` function that dynamically routes event logging
//! to the appropriate backend (Borsh vs Zero-Copy) based on the active feature flags.

#[cfg(all(
    feature = "borsh",
    not(feature = "pinocchio"),
    not(feature = "wincode")
))]
mod borsh;
#[cfg(feature = "pinocchio")]
mod pinocchio;
#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
mod wincode;
#[cfg(all(
    not(feature = "pinocchio"),
    not(any(feature = "borsh", feature = "wincode"))
))]
mod zero_copy;

#[cfg(all(
    feature = "borsh",
    not(feature = "pinocchio"),
    not(feature = "wincode")
))]
pub fn emit_event<T: crate::borsh::BorshSerialize>(event: &T, discriminator: &[u8]) {
    borsh::emit(event, discriminator);
}

#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
pub fn emit_event<T: crate::wincode::SchemaWrite<crate::wincode_config::Config, Src = T>>(
    event: &T,
    discriminator: &[u8],
) {
    wincode::emit(event, discriminator);
}

#[cfg(all(
    not(feature = "pinocchio"),
    not(any(feature = "borsh", feature = "wincode"))
))]
pub fn emit_event<T: bytemuck::Pod>(event: &T, discriminator: &[u8]) {
    zero_copy::emit(event, discriminator);
}

#[cfg(feature = "pinocchio")]
pub fn emit_event<T: bytemuck::Pod>(event: &T, discriminator: &[u8]) {
    pinocchio::emit(event, discriminator);
}

/// Joined self-CPI payloads up to this many bytes are built on the stack (no
/// heap allocation); a fixed-size zero-copy event is far below it. Uninitialised
/// stack costs nothing to reserve, so the size only bounds frame usage, which
/// SBF caps at 4 KiB per frame.
const EVENT_CPI_STACK_BYTES: usize = 512;

/// Calls `send` with `parts` joined into one contiguous slice. Joins on the
/// stack when it fits `EVENT_CPI_STACK_BYTES`, otherwise panics.
#[inline(always)]
pub fn with_concat<R>(parts: &[&[u8]], send: impl FnOnce(&[u8]) -> R) -> R {
    match try_with_concat(parts, send) {
        Some(result) => result,
        None => panic!("event too large"),
    }
}

/// Like [`with_concat`], but returns `None` without calling `send` when the
/// joined length exceeds `EVENT_CPI_STACK_BYTES`, so the caller can fall back.
#[inline(always)]
pub fn try_with_concat<R>(parts: &[&[u8]], send: impl FnOnce(&[u8]) -> R) -> Option<R> {
    let mut total: usize = 0;
    for part in parts {
        total = total.checked_add(part.len())?;
    }
    if total > EVENT_CPI_STACK_BYTES {
        return None;
    }
    let mut buffer = [core::mem::MaybeUninit::<u8>::uninit(); EVENT_CPI_STACK_BYTES];
    let base = buffer.as_mut_ptr().cast::<u8>();
    let mut offset = 0;
    for part in parts {
        // SAFETY: `offset + part.len() <= total <= EVENT_CPI_STACK_BYTES`,
        // and `part` cannot overlap this local buffer.
        unsafe { core::ptr::copy_nonoverlapping(part.as_ptr(), base.add(offset), part.len()) };
        offset += part.len();
    }
    // SAFETY: the copies above initialised exactly `buffer[..total]`.
    Some(send(unsafe { core::slice::from_raw_parts(base, total) }))
}

/// A stack buffer of exactly `LEN` bytes, filled front to back by `push`/
/// `push_zeros` and readable only once every byte has been written. Used to
/// log an event as one `sol_log_data` slice (the runtime charges per slice)
/// without first building a second copy of the event.
///
/// Invariant: `buf[..len]` is initialized and `len <= LEN`. Every method
/// preserves it, and `finish` only exposes `buf` when `len == LEN`.
pub struct EventFrame<const LEN: usize> {
    buf: core::mem::MaybeUninit<[u8; LEN]>,
    len: usize,
}

impl<const LEN: usize> Default for EventFrame<LEN> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<const LEN: usize> EventFrame<LEN> {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            buf: core::mem::MaybeUninit::uninit(),
            len: 0,
        }
    }

    /// Appends `bytes`. Panics if they would not fit in the remaining space.
    #[inline(always)]
    pub fn push(&mut self, bytes: &[u8]) {
        assert!(bytes.len() <= LEN - self.len, "event frame overflow");
        // SAFETY: `self.len + bytes.len() <= LEN` (checked above), so the copy
        // stays inside `buf`; `bytes` is a caller borrow and cannot overlap it.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.buf.as_mut_ptr().cast::<u8>().add(self.len),
                bytes.len(),
            );
        }
        self.len += bytes.len();
    }

    /// Appends `count` zero bytes. Panics if they would not fit.
    #[inline(always)]
    pub fn push_zeros(&mut self, count: usize) {
        assert!(count <= LEN - self.len, "event frame overflow");
        // SAFETY: `self.len + count <= LEN` (checked above).
        unsafe {
            core::ptr::write_bytes(self.buf.as_mut_ptr().cast::<u8>().add(self.len), 0, count);
        }
        self.len += count;
    }

    /// The full frame. Panics unless all `LEN` bytes have been written.
    #[inline(always)]
    pub fn finish(&self) -> &[u8] {
        assert!(self.len == LEN, "event frame incomplete");
        // SAFETY: `len == LEN`, and by the type invariant `buf[..len]` is
        // initialized, so the whole buffer is.
        unsafe { self.buf.assume_init_ref() }
    }
}

#[cfg(kani)]
mod event_frame_kani_proofs {
    use super::*;

    /// For any discriminator, payload and padding that exactly fill the frame,
    /// `finish` returns precisely their concatenation, in order.
    #[kani::proof]
    #[kani::unwind(9)]
    fn prove_event_frame_bytes_are_exact_concatenation() {
        let disc: [u8; 3] = kani::any();
        let payload: [u8; 4] = kani::any();
        let mut frame = EventFrame::<8>::new();
        frame.push(&disc);
        frame.push(&payload);
        frame.push_zeros(1);
        let out = frame.finish();
        assert!(out.len() == 8);
        let mut i = 0;
        while i < 3 {
            assert!(out[i] == disc[i]);
            i += 1;
        }
        while i < 7 {
            assert!(out[i] == payload[i - 3]);
            i += 1;
        }
        assert!(out[7] == 0);
    }

    /// Any split of lengths that fits is written without panicking and
    /// without leaving the buffer, and the cursor advances by exactly that much.
    #[kani::proof]
    fn prove_event_frame_in_bounds_pushes_succeed() {
        let a: usize = kani::any();
        let b: usize = kani::any();
        kani::assume(a <= 8 && b <= 8 - a);
        let bytes = [0u8; 8];
        let mut frame = EventFrame::<8>::new();
        frame.push(&bytes[..a]);
        frame.push_zeros(b);
        assert!(frame.len == a + b);
    }

    /// A push that would overflow the buffer panics instead of writing past it.
    #[kani::proof]
    #[kani::should_panic]
    fn prove_event_frame_overflowing_push_panics() {
        let a: usize = kani::any();
        let b: usize = kani::any();
        kani::assume(a <= 8 && b <= 16 && a + b > 8);
        let bytes = [0u8; 8];
        let mut frame = EventFrame::<8>::new();
        frame.push(&bytes[..a]);
        frame.push_zeros(b);
    }

    /// `finish` refuses to expose a frame with any unwritten byte.
    #[kani::proof]
    #[kani::should_panic]
    fn prove_event_frame_finish_rejects_incomplete_frame() {
        let n: usize = kani::any();
        kani::assume(n < 8);
        let mut frame = EventFrame::<8>::new();
        frame.push_zeros(n);
        let _ = frame.finish();
    }
}

/// Implemented by every `#[event]` type: hands `send` the exact self-CPI
/// instruction data `emit_cpi!` sends — `tag ++ this event's own discriminator
/// ++ its fields`, with the fields in the same encoding its plain `emit()`
/// logs. A fixed-size zero-copy event is joined on the stack; an event with
/// dynamic fields (Borsh, `#[event(alloc)]`) needs its one heap buffer anyway.
pub trait EventCpiPayload {
    fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R;
}

/// Records `event` in the transaction's inner-instruction trace by invoking
/// the current program with `event_authority` (the program's own PDA) as the
/// sole signer. The authority and its bump come from the handler's accounts
/// type (`EventCpiAuthority`, implemented by `#[event_cpi]`) and the program's
/// reserved event-CPI dispatch tag from `EventCpiTag`; the program's receive
/// arm accepts the call only when `event_authority` signed for real.
/// Returns nothing: the runtime ends the program on any CPI failure instead of
/// returning an error code to it, so a returning call has succeeded.
#[cfg(not(feature = "pinocchio"))]
pub fn emit_cpi<
    A: crate::prelude::EventCpiAuthority + crate::prelude::EventCpiTag,
    T: EventCpiPayload,
>(
    ctx: &crate::prelude::Context<A>,
    event: &T,
) {
    let event_authority = ctx.accounts.event_authority();
    let bump = A::event_authority_bump(&ctx.bumps);
    event.with_cpi_payload(A::EVENT_DISPATCH_TAG, |data| {
        use crate::cpi::stable::{invoke_signed_raw, StableAccountMeta, StableInstruction};
        // The only account is a read-only signer, so `invoke_signed`'s
        // `RefCell` borrow checks (writable accounts only) have nothing to
        // check and the raw path is equivalent.
        let accounts = [StableAccountMeta {
            pubkey: event_authority.address(),
            is_signer: true,
            is_writable: false,
        }];
        let instruction = StableInstruction::new(*ctx.program_id, &accounts, data);
        let account_infos = [unsafe { event_authority.to_lifetime() }];
        invoke_signed_raw(
            &instruction,
            &account_infos,
            &[&[b"__event_authority", &[bump]]],
        )
        .unwrap_or_else(|_| panic!("event self-CPI returned an error code"))
    })
}

#[cfg(feature = "pinocchio")]
pub fn emit_cpi<
    A: crate::prelude::EventCpiAuthority + crate::prelude::EventCpiTag,
    T: EventCpiPayload,
>(
    ctx: &crate::prelude::Context<A>,
    event: &T,
) {
    let event_authority = ctx.accounts.event_authority();
    let bump = A::event_authority_bump(&ctx.bumps);
    event.with_cpi_payload(A::EVENT_DISPATCH_TAG, |data| {
        let accounts = [
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                event_authority.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: ctx.program_id.as_address(),
            accounts: &accounts,
            data,
        };
        let bump_slice: &[u8] = core::slice::from_ref(&bump);
        let seeds = [
            ::pinocchio::cpi::Seed::from(b"__event_authority".as_ref()),
            ::pinocchio::cpi::Seed::from(bump_slice),
        ];
        let signer = [::pinocchio::cpi::Signer::from(seeds.as_ref())];
        let cpi_account = ::pinocchio::cpi::CpiAccount::from(&event_authority.view);
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(&instruction, &[cpi_account], &signer);
        }
    })
}

#[cfg(test)]
mod event_frame_tests {
    use super::*;

    #[test]
    fn frame_matches_concatenation() {
        let mut frame = EventFrame::<7>::new();
        frame.push(&[1, 2]);
        frame.push(&[3, 4, 5]);
        frame.push_zeros(2);
        assert!(frame.finish() == [1, 2, 3, 4, 5, 0, 0]);
    }

    #[test]
    fn empty_frame_is_complete() {
        let frame = EventFrame::<0>::new();
        assert!(frame.finish().is_empty());
    }

    #[test]
    #[should_panic(expected = "event frame incomplete")]
    fn incomplete_frame_panics() {
        let mut frame = EventFrame::<4>::new();
        frame.push(&[1, 2, 3]);
        let _ = frame.finish();
    }

    #[test]
    #[should_panic(expected = "event frame overflow")]
    fn overflowing_push_panics() {
        let mut frame = EventFrame::<2>::new();
        frame.push(&[1, 2, 3]);
    }

    #[test]
    fn try_with_concat_joins_parts() {
        let mut out = [0u8; 3];
        let joined = try_with_concat(&[&[1u8, 2][..], &[3u8][..]], |bytes| {
            out.copy_from_slice(bytes);
            bytes.len()
        });
        assert!(joined == Some(3) && out == [1, 2, 3]);
    }

    #[test]
    fn try_with_concat_declines_oversized_input() {
        let big = [0u8; EVENT_CPI_STACK_BYTES + 1];
        assert!(try_with_concat(&[&big[..]], |_| ()).is_none());
    }
}
