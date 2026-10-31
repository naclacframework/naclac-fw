use naclac_lang::prelude::*;

// `core/src/wrappers/zc_vec.rs`'s `ZcString` is the zero-copy
// replacement for `String` — but only ever as an *instruction argument*
// (parsed fresh out of the current transaction's instruction-data buffer,
// used and discarded within that same call). It is NOT a valid
// `#[account]` field type: `macros/src/account.rs`'s own
// zero-copy branch explicitly rejects `String`/`Vec<T>` fields with a
// compile error (a `ZcVec<T>`/`ZcString` is a raw pointer + length; a
// pointer value written into on-chain bytes in one transaction is
// meaningless — or attacker-controlled — when read back in a later one).
//
// So a *persisted* string has to live in a fixed-size byte array instead,
// exactly as `account.rs`'s own error message suggests. `Note` stores up
// to 32 bytes plus an explicit length so the real byte content survives a
// write/read round trip without ever putting a pointer in account data.
#[account]
pub struct Note {
    pub bump: u8,
    pub len: u8,
    pub message: [u8; 32],
}
