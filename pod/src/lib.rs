//! # naclac-pod
//!
//! Plain-data types for zero-copy account layouts. Every type here has
//! alignment 1 and no padding, so a struct built from them (and from plain
//! integers, in a `#[repr(C, packed)]` struct) can be read from and written to
//! account bytes at any address with no copy.
//!
//! - [`Bool`]: a flag byte that is valid for every byte value.
//! - [`Opt`]: an optional value, tag byte first.
//! - [`FixedVec`] / [`FixedString`]: inline lists and strings of fixed
//!   capacity, with a two-byte length.
//! - [`ZcEnum`]: an enum stored as its tag and the active variant's fields,
//!   packed; [`ZcCodec`] is the stored form of one value.
//! - [`ZcVec`] / [`ZcString`]: views into instruction data, for arguments only.
//! - [`Unaligned`]: marks the element types of alignment 1, whose lists can be
//!   borrowed as slices.
//! - [`PodError`]: the errors those types return, convertible into a program
//!   error under the `solana` or `pinocchio` feature.
//!
//! The crate is `no_std` and has no dependency beyond `bytemuck` (plus the
//! optional ones that provide the error and address conversions). It is tested
//! on the host (including under Miri) and verified with Kani proofs
//! (`cargo kani -p naclac-pod`).

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(test)]
extern crate std;

mod bool_;
mod error;
mod fixed_string;
mod fixed_vec;
mod opt;
mod unaligned;
mod zc_enum;
mod zc_vec;

#[cfg(kani)]
mod proofs;

pub use {
    bool_::{Bool, BoolField},
    error::{PodError, CAPACITY_EXCEEDED_CODE, INVALID_INSTRUCTION_DATA_CODE, INVALID_UTF8_CODE},
    fixed_string::FixedString,
    fixed_vec::{FixedVec, Iter as FixedVecIter},
    opt::Opt,
    unaligned::Unaligned,
    zc_enum::{
        decode_pod, encode_pod, event_bytes_of, event_field_default, event_slice_bytes, ZcCodec,
        ZcEnum, ZcEnumKind, ZcEventDefault, ZcEventField, ZcField, ZcStorage,
    },
    zc_vec::{ZcString, ZcVec, ZcVecIter},
};
