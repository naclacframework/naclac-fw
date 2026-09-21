//! # Instruction Macro Module
//!
//! Houses the submodules that generate an instruction's security checks,
//! CPI initializers, and account reallocation logic, shared by
//! `#[derive(Accounts)]`'s own codegen.

pub mod close_account;
pub mod init_cpi;
pub mod parser;
pub mod realloc;
pub mod security;
