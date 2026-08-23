//! # Borsh Event Emission
//!
//! Handles event logging for standard Solana programs using heap-allocated `Vec`s
//! and Borsh serialization.

use crate::prelude::borsh::{self, BorshSerialize};

pub fn emit<T: BorshSerialize>(event: &T, discriminator: [u8; 8]) {
    let mut data = discriminator.to_vec();
    let serialized_payload = borsh::to_vec(event)
        .expect("event serialization is infallible for all currently-supported field types");
    data.extend(serialized_payload);
    crate::prelude::sol_log_data(&[&data]);
}
