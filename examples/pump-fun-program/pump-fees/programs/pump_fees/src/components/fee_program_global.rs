use naclac_lang::prelude::*;

// Fields ordered so no internally-aligned field ever follows a non-multiple-of-8
// offset (avoids an internal padding gap between fields, which a tightly-packed
// TS decoder can't skip over — trailing padding at the struct's end is fine).
#[component]
pub struct FeeProgramGlobal {
    pub claim_rate_limit: u64,
    pub authority: Address,
    pub social_claim_authority: Address,
    pub bump: u8,
    pub disable_flags: u8,
    pub _reserved: [u8; 256],
}
