use naclac_lang::prelude::*;

// Deliberately small and fixed — `realloc` changes the account's raw byte
// length, not this struct's own compile-time size; the extra allocated
// bytes beyond `size_of::<Growable>()` are just reserved space the
// instruction handler can (in a real program) read/write manually. The
// test only cares about the account's actual on-chain byte length and
// lamports, not this struct's fields.
#[component]
pub struct Growable {
    pub bump: u8,
    pub tag: u32,
}
