use naclac_lang::prelude::*;

// user_id: real type is a Borsh String (max 20 chars); stored here as an
// explicit u32 length prefix + fixed [u8; 20] content buffer, since zero-copy
// #[component]s reject String. Fields ordered so no internally-aligned field
// ever follows a non-multiple-of-8 offset (avoids an internal padding gap
// between fields, which a tightly-packed TS decoder can't skip over —
// trailing padding at the struct's end is fine).
/// Platform identifier: 0=pump, 1=twitter, etc.
#[component]
pub struct SocialFeePda {
    pub total_claimed: u64,
    pub last_claimed: u64,
    pub total_stable_claimed: u64,
    /// Max 20 characters to fit u64::MAX (18,446,744,073,709,551,615) as a string.
    /// Actual storage: 4 bytes (length prefix) + 20 bytes (content) = 24 bytes.
    pub user_id_len: u32,
    pub bump: u8,
    pub version: u8,
    pub platform: u8,
    pub user_id: [u8; 20],
    pub _reserved: [u8; 120],
}
