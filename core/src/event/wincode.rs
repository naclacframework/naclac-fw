//! # Wincode Event Emission
//!
//! Handles event logging for programs using heap-allocated `Vec`s
//! and Wincode serialization.

use crate::{
    prelude::wincode::{self, SchemaWrite},
    wincode_config::{Config, CONFIG},
};

/// Logs `discriminator` followed by the Wincode encoding of `event` as a
/// single data field, using one allocation of exactly that size. Errors use a
/// static panic message rather than `.expect(...)`, which would need
/// `WriteError: Debug` and pull the general formatting machinery into the
/// program.
pub fn emit<T: SchemaWrite<Config, Src = T>>(event: &T, discriminator: &[u8]) {
    let payload_len = wincode::config::serialized_size(event, CONFIG).unwrap_or_else(|_| {
        panic!("event serialization is infallible for all currently-supported field types")
    });
    let mut data = crate::prelude::Vec::with_capacity(
        discriminator.len().saturating_add(payload_len as usize),
    );
    data.extend_from_slice(discriminator);
    wincode::config::serialize_into(&mut data, event, CONFIG).unwrap_or_else(|_| {
        panic!("event serialization is infallible for all currently-supported field types")
    });
    crate::prelude::sol_log_data(&[&data]);
}
