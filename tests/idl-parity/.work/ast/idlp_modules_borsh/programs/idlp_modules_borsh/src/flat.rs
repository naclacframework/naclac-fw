//! A flat (non-`mod.rs`) file module whose child lives in `flat/`. `FLAT_SEED` is untagged and
//! reaches the IDL only through its use as a seed.

pub mod child;

pub const FLAT_SEED: &[u8] = b"flat";
