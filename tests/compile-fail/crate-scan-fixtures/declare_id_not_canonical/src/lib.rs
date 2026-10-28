//! Must FAIL: the declared address is 41 `1`s, which is not the canonical
//! base58 encoding of 32 bytes.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111111111111");
