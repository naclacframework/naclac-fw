//! # Wincode Event Emission
//!
//! Builds the frame an event is logged or self-CPI'd as: the CPI dispatch tag
//! (empty for a log), the event discriminator, then the Wincode encoding of
//! the event. An event whose encoding always has the same size is framed on the
//! stack; an event with dynamic fields is framed in one exactly-sized heap
//! buffer.

use {
    crate::{
        prelude::wincode::{self, SchemaWrite, TypeMeta},
        wincode_config::{Config, CONFIG},
    },
    core::marker::PhantomData,
};

/// The most bytes a stack frame holds.
const MAX_FRAME_BYTES: usize = 1024;

/// The most bytes the CPI dispatch tag and the discriminator take together:
/// each is at most 8 bytes.
const MAX_PREFIX_BYTES: usize = 16;

#[cold]
fn encode_failed() -> ! {
    panic!("event serialization is infallible for all currently-supported field types")
}

struct Framing<T>(PhantomData<T>);

impl<T: SchemaWrite<Config, Src = T>> Framing<T> {
    /// True when every value of `T` encodes to the same number of bytes and a
    /// frame around it fits `MAX_FRAME_BYTES`. A compile-time property of the
    /// type, so only the branch that applies to it is compiled in.
    const ON_STACK: bool = match <T as SchemaWrite<Config>>::TYPE_META {
        TypeMeta::Static { size, .. } => size <= MAX_FRAME_BYTES - MAX_PREFIX_BYTES,
        _ => false,
    };
}

/// Logs `discriminator` followed by the Wincode encoding of `event` as a
/// single data field.
#[inline(never)]
pub fn emit<T: SchemaWrite<Config, Src = T>>(event: &T, discriminator: &[u8]) {
    with_frame(&[], discriminator, event, |frame| {
        crate::prelude::sol_log_data(&[frame]);
    });
}

/// Calls `send` with `tag`, then `discriminator`, then the Wincode encoding of
/// `event`, as one contiguous slice. Failures panic with a static message,
/// since a `.expect(...)` would need `WriteError: Debug` and pull the general
/// formatting machinery into the program.
pub fn with_frame<T: SchemaWrite<Config, Src = T>, R>(
    tag: &[u8],
    discriminator: &[u8],
    event: &T,
    send: impl FnOnce(&[u8]) -> R,
) -> R {
    if Framing::<T>::ON_STACK {
        let mut buffer = [0u8; MAX_FRAME_BYTES];
        let Some(frame) = encode(event, tag, discriminator, &mut buffer) else {
            encode_failed()
        };
        send(frame)
    } else {
        let payload_len =
            wincode::config::serialized_size(event, CONFIG).unwrap_or_else(|_| encode_failed());
        let mut data = crate::prelude::Vec::with_capacity(
            tag.len()
                .saturating_add(discriminator.len())
                .saturating_add(payload_len as usize),
        );
        data.extend_from_slice(tag);
        data.extend_from_slice(discriminator);
        wincode::config::serialize_into(&mut data, event, CONFIG)
            .unwrap_or_else(|_| encode_failed());
        send(&data)
    }
}

/// `tag`, `discriminator` and the Wincode encoding of `event`, written in that
/// order at the start of `buffer`. `None` if they do not fit or cannot be
/// encoded.
fn encode<'a, T: SchemaWrite<Config, Src = T>>(
    event: &T,
    tag: &[u8],
    discriminator: &[u8],
    buffer: &'a mut [u8],
) -> Option<&'a [u8]> {
    let mut rest: &mut [u8] = &mut *buffer;
    for part in [tag, discriminator] {
        let (head, tail) = core::mem::take(&mut rest).split_at_mut_checked(part.len())?;
        for (slot, byte) in head.iter_mut().zip(part) {
            *slot = *byte;
        }
        rest = tail;
    }
    let payload_len =
        usize::try_from(wincode::config::serialized_size(event, CONFIG).ok()?).ok()?;
    let payload = rest.get_mut(..payload_len)?;
    wincode::config::serialize_into(payload, event, CONFIG).ok()?;
    buffer.get(
        ..tag
            .len()
            .saturating_add(discriminator.len())
            .saturating_add(payload_len),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_is_the_tag_then_the_discriminator_then_the_wincode_encoding() {
        let mut buffer = [0xEEu8; 32];
        let frame = encode(&0x0102_0304u32, &[7], &[9, 8], &mut buffer).unwrap();
        let encoded = wincode::config::serialize(&0x0102_0304u32, CONFIG).unwrap();
        assert!(frame == [7, 9, 8, 4, 3, 2, 1]);
        assert!(frame[3..] == encoded[..]);
    }

    #[test]
    fn empty_prefixes_leave_only_the_encoding() {
        let mut buffer = [0u8; 16];
        let frame = encode(&0x0102_0304_0506_0708u64, &[], &[], &mut buffer).unwrap();
        assert!(frame == 0x0102_0304_0506_0708u64.to_le_bytes());
    }

    #[test]
    fn a_frame_that_does_not_fit_is_none_and_one_that_just_fits_is_not() {
        assert!(encode(&1u64, &[], &[1, 2], &mut [0u8; 9]).is_none());
        assert!(encode(&1u64, &[], &[1, 2], &mut [0u8; 10]).is_some());
        assert!(encode(&1u64, &[3], &[1, 2], &mut [0u8; 10]).is_none());
        assert!(encode(&1u64, &[], &[1, 2, 3], &mut [0u8; 2]).is_none());
    }

    #[test]
    fn only_fixed_size_types_are_framed_on_the_stack() {
        const {
            assert!(Framing::<u64>::ON_STACK);
            assert!(Framing::<[u8; 32]>::ON_STACK);
            assert!(!Framing::<crate::prelude::Vec<u8>>::ON_STACK);
            assert!(!Framing::<[u8; MAX_FRAME_BYTES]>::ON_STACK);
        }
    }

    #[test]
    fn both_framings_produce_the_same_bytes() {
        let mut seen = None;
        with_frame(&[1], &[2, 3], &0xAABB_CCDDu32, |frame| {
            seen = Some(frame.to_vec())
        });
        assert!(seen.unwrap() == [1, 2, 3, 0xDD, 0xCC, 0xBB, 0xAA]);

        let mut seen = None;
        with_frame(
            &[1],
            &[2, 3],
            &crate::prelude::Vec::from([5u8, 6]),
            |frame| seen = Some(frame.to_vec()),
        );
        assert!(seen.unwrap() == [1, 2, 3, 2, 0, 0, 0, 5, 6]);
    }
}
