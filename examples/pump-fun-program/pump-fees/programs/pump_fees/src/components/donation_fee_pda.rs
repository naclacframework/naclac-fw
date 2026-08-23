use naclac_lang::prelude::*;

// Fields ordered so the two 8-byte-aligned fields (`total_donated`,
// `last_crank_ts`) come first, before the byte-array fields — same
// padding-avoidance rationale as `SocialFeePda`. The real program (Borsh,
// no alignment concerns) uses a different order; this is a separate
// deployment, not a shared decoder, so byte-for-byte parity isn't required.
/// Escrow PDA for donation relay: one per (mint, donation campaign `config_id`).
#[component]
pub struct DonationFeePda {
    pub total_donated: u64,
    pub last_crank_ts: i64,
    pub config_id: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub creator: Address,
    pub bump: u8,
    pub version: u8,
    pub _reserved: [u8; 64],
}
