//! # Borsh Event Emission
//!
//! Handles event logging for standard Solana programs using heap-allocated `Vec`s
//! and Borsh serialization.

use crate::prelude::borsh::{self, BorshSerialize};

/// Logs `discriminator` followed by the Borsh encoding of `event` as a single
/// data field, using one allocation of exactly that size.
pub fn emit<T: BorshSerialize>(event: &T, discriminator: &[u8]) {
    let payload_len = borsh::object_length(event).unwrap_or_else(|_| {
        panic!("event serialization is infallible for all currently-supported field types")
    });
    let mut data =
        crate::prelude::Vec::with_capacity(discriminator.len().saturating_add(payload_len));
    data.extend_from_slice(discriminator);
    borsh::to_writer(&mut data, event).unwrap_or_else(|_| {
        panic!("event serialization is infallible for all currently-supported field types")
    });
    crate::prelude::sol_log_data(&[&data]);
}
