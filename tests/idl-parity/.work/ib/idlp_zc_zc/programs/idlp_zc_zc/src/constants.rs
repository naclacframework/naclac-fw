//! Constants covering every IDL constant type and every way a constant reaches a seed
//! or an array length. Tagged (`#[constant]`) and untagged constants are mixed on purpose.

use naclac_lang::prelude::*;

/// Byte-string seed, tagged.
#[constant]
pub const SEED_VAULT: &[u8] = b"vault";

pub const SEED_REGISTRY: &[u8] = b"registry";
pub const SEED_ENTRY: &[u8] = b"entry";
pub const SEED_LEDGER: [u8; 6] = *b"ledger";
pub const SEED_TEXT: &str = "text_seed";
pub const SEED_GROW: &[u8] = b"grow";
pub const SEED_EXTERNAL: &[u8] = b"external";

#[constant]
pub const ARR_LEN: usize = 4;

#[constant]
pub const FEE_BPS: u16 = 250;

#[constant]
pub const VERSION: u8 = 3;

/// Underscore separators in the literal.
#[constant]
pub const FEE_DENOMINATOR: u64 = 10_000;

#[constant]
pub const NEGATIVE: i32 = -5;

#[constant]
pub const HEX: u32 = 0xFF;

#[constant]
pub const SUFFIXED: u32 = 7u32;

#[constant]
pub const RATE: f64 = 1.5;

/// A computed value: only evaluation can produce it.
#[constant]
pub const COMPUTED: u64 = 2 * 5;

#[constant]
pub const ENABLED: bool = true;

#[constant]
pub const LABEL: &str = "label";

#[constant]
pub const TAG: [u8; 4] = *b"abcd";

#[constant]
pub const TABLE: [u16; 3] = [1, 2, 3];

#[constant]
pub const TREASURY: Address = address!("11111111111111111111111111111111");

pub const OTHER_PROGRAM: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
