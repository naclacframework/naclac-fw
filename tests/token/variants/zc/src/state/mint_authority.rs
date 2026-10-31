use naclac_lang::prelude::*;

// A PDA that owns every mint/vault this test case creates — the CPI-signing
// authority for `mint_to_signed`/`transfer`, mirroring
// `examples/launchpad`'s `LaunchRecord` role exactly.
#[account]
pub struct MintAuthority {
    pub bump: u8,
}
