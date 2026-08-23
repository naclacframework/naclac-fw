use naclac_lang::prelude::*;

#[error_code]
pub enum DonationRelayError {
    /// `message` exceeds the real on-chain limit of 255 bytes
    InvalidMessageLength,
    /// Accumulating `debouncer.total_amount` overflowed u64
    MathOverflow,
}
