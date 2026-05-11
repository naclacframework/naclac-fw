//! # Zero-Copy Event Emission (Pinocchio)
//!
//! Handles event logging for Pinocchio `no_std` programs.
//! Emits events directly via `sol_log_data` using `bytemuck` slice casting, completely 
//! avoiding heap allocation.

use bytemuck::Pod;

pub fn emit<T: Pod>(event: &T, discriminator: [u8; 8]) {
    let data = bytemuck::bytes_of(event);
    crate::prelude::sol_log_data(&[&discriminator, data]);
}
