//! # Event Emission Routing
//!
//! Exposes a unified `emit_event` function that dynamically routes event logging
//! to the appropriate backend (Borsh vs Zero-Copy) based on the active feature flags.

#[cfg(all(not(feature = "pinocchio"), not(feature = "zero-copy")))]
pub mod borsh;
#[cfg(all(not(feature = "pinocchio"), feature = "zero-copy"))]
pub mod zero_copy;
#[cfg(feature = "pinocchio")]
pub mod pinocchio;

#[cfg(all(not(feature = "pinocchio"), not(feature = "zero-copy")))]
pub fn emit_event<T: crate::borsh::BorshSerialize>(event: &T, discriminator: [u8; 8]) {
    borsh::emit(event, discriminator);
}

#[cfg(all(not(feature = "pinocchio"), feature = "zero-copy"))]
pub fn emit_event<T: bytemuck::Pod>(event: &T, discriminator: [u8; 8]) {
    zero_copy::emit(event, discriminator);
}

#[cfg(feature = "pinocchio")]
pub fn emit_event<T: bytemuck::Pod>(event: &T, discriminator: [u8; 8]) {
    pinocchio::emit(event, discriminator);
}
