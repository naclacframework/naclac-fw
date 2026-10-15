//! Project-wide `wincode` serialization configuration.
//!
//! wincode's crate-root `serialize`/`deserialize` functions are hardcoded to
//! its bincode-compatible `DefaultConfig` (4-byte enum tags, 8-byte sequence
//! lengths). naclac's other two backends don't use those widths: Borsh uses
//! a 1-byte enum tag and a 4-byte sequence length, and the zero-copy backend
//! defaults every `#[defined_type]` enum to a 1-byte (`#[repr(u8)]`)
//! discriminant. `Config` matches that existing convention instead of
//! wincode's bincode-compatible default, so every naclac call site
//! (`naclac-core`, `naclac-macros`-generated code) must go through the
//! `wincode::config::*` functions with this value rather than the
//! `DefaultConfig`-bound crate-root functions, or the two config surfaces
//! will silently disagree on wire format.

#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
pub type Config = wincode::config::Configuration<
    true,
    { wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT },
    wincode::len::FixIntLen<u32>,
    wincode::int_encoding::LittleEndian,
    wincode::int_encoding::FixInt,
    u8,
>;

#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
pub const CONFIG: Config = wincode::config::Configuration::default()
    .with_length_encoding::<wincode::len::FixIntLen<u32>>()
    .with_tag_encoding::<u8>();
