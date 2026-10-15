//! # Zero-Copy Event Emission (Solana)
//!
//! Handles event logging for standard Solana programs configured for zero-copy.
//! Emits events directly via `sol_log_data` using `bytemuck` slice casting, completely
//! avoiding heap allocation.

use bytemuck::Pod;

pub fn emit<T: Pod>(event: &T, discriminator: &[u8]) {
    let data = bytemuck::bytes_of(event);
    let logged = super::try_with_concat(&[discriminator, data], |frame| {
        crate::prelude::sol_log_data(&[frame]);
    });
    if logged.is_none() {
        crate::prelude::sol_log_data(&[discriminator, data]);
    }
}
