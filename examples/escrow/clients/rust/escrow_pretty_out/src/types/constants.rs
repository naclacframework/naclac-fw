#[cfg(feature = "offchain")]
pub const PROGRAM_ID: naclac_client::Address = naclac_client::Address::new_from_array([
    173, 102, 136, 71, 225, 42, 117, 123, 74, 179, 241, 91, 83, 151, 195, 64, 18, 171,
    32, 189, 199, 253, 100, 119, 165, 207, 89, 225, 97, 12, 2, 152,
]);
#[cfg(not(feature = "offchain"))]
pub const PROGRAM_ID: crate::sdk_core::Address = crate::sdk_core::Address::new_from_array([
    173, 102, 136, 71, 225, 42, 117, 123, 74, 179, 241, 91, 83, 151, 195, 64, 18, 171,
    32, 189, 199, 253, 100, 119, 165, 207, 89, 225, 97, 12, 2, 152,
]);
pub const SEED_ESCROW: &[u8] = &[101, 115, 99, 114, 111, 119];
