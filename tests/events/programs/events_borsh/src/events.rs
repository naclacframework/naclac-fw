use naclac_lang::prelude::*;

// One field only, deliberately: the point of this test is confirming the
// event discriminator + payload round-trip through each backend's real
// `sol_log_data` encoding, not exercising a rich payload shape.
#[event]
pub struct CounterIncremented {
    pub new_count: u64,
}

/// Variable-length payload for the self-CPI vs. `sol_log_data` CU sweep —
/// `data`'s length is the swept variable.
#[event]
pub struct SizedPayload {
    pub data: Vec<u8>,
}
