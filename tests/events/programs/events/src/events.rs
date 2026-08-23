use naclac_lang::prelude::*;

// One field only, deliberately: the point of this test is confirming the
// event discriminator + payload round-trip through each backend's real
// `sol_log_data` encoding, not exercising a rich payload shape.
#[event]
pub struct CounterIncremented {
    pub new_count: u64,
}

/// Variable-length payload for the self-CPI vs. `sol_log_data` CU sweep —
/// `data`'s length is the swept variable. `alloc` mode is required here:
/// pinocchio is always zero-copy, so a `Vec<u8>` field needs the
/// single-allocation buffer path rather than `Pod`/`bytemuck`.
#[event(alloc)]
pub struct SizedPayload {
    pub data: Vec<u8>,
}

// Fixed-size counterparts to `SizedPayload`, for the plain-`#[event]`
// (`bytemuck::bytes_of`, no allocation) CU sweep — a `[u8; N]`'s `N` is
// fixed per type, so each size needs its own struct rather than one
// runtime-parameterized instruction.
#[event]
pub struct FixedPayload8 {
    pub data: [u8; 8],
}

#[event]
pub struct FixedPayload128 {
    pub data: [u8; 128],
}

#[event]
pub struct FixedPayload512 {
    pub data: [u8; 512],
}

#[event]
pub struct FixedPayload2048 {
    pub data: [u8; 2048],
}
