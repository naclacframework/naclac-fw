//! Builds only for a valid runtime and layout. Each test in `../../tests/diagnostics.rs`
//! builds this crate with one conflicting feature set and asserts the error
//! `naclac-core` or `naclac-lang` reports.
#![no_std]
