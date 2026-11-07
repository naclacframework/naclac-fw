//! Everything specific to one field of an Accounts struct: parsing its
//! `#[account(...)]` attribute (`parser`), the checks and `init`/`realloc`/
//! `close` code generated from it (`checks`, `init_cpi`, `realloc`,
//! `close_account`), the order fields load in (`load_order`), the builtin
//! constraint namespaces (`builtin_namespaces`) and lookups in the calling
//! crate's source (`compile_time`).

pub mod builtin_namespaces;
pub mod checks;
pub mod close_account;
pub mod compile_time;
pub mod init_cpi;
pub mod load_order;
pub mod parser;
pub mod realloc;
