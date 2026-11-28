use naclac_lang::prelude::*;

/// One field only, deliberately: the point of this test is confirming the
/// event discriminator + payload round-trip through each backend's real
/// `sol_log_data` encoding, not exercising a rich payload shape.
#[event]
pub struct CounterIncremented {
    pub new_count: u64,
}

#[event]
pub struct BatchTouched {
    pub tag: u64,
    pub values: [u64; 8],
}

// Fixed-size payloads for the plain-`#[event]` (`bytemuck::bytes_of`, no
// allocation) CU sweep — a `[u8; N]`'s `N` is fixed per type, so each size
// needs its own struct rather than one runtime-parameterized instruction.
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

#[event]
pub struct RosterChanged {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<16>,
}
