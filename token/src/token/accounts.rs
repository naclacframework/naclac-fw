//! Account bundles passed to the `TokenCpi` trait methods.

#[derive(Debug)]
pub struct TransferCheckedAccounts<'a, A, B, C, D> {
    pub from: &'a mut A,
    pub mint: &'a B,
    pub to: &'a mut C,
    pub authority: &'a D,
}

#[derive(Debug)]
pub struct MintToAccounts<'a, A, B, C> {
    pub mint: &'a mut A,
    pub to: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct BurnAccounts<'a, A, B, C> {
    pub mint: &'a mut A,
    pub from: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct CloseAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub destination: &'a mut B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct ApproveAccounts<'a, A, B, C> {
    pub to: &'a mut A,
    pub delegate: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct RevokeAccounts<'a, A, B> {
    pub source: &'a mut A,
    pub authority: &'a B,
}

#[derive(Debug)]
pub struct InitializeMintAccounts<'a, A> {
    pub mint: &'a mut A,
}

#[derive(Debug)]
pub struct SyncNativeAccounts<'a, A> {
    pub native_token: &'a mut A,
}

#[derive(Debug)]
pub struct InitializeAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct FreezeAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct ThawAccountAccounts<'a, A, B, C> {
    pub account: &'a mut A,
    pub mint: &'a B,
    pub authority: &'a C,
}

#[derive(Debug)]
pub struct SetAuthorityAccounts<'a, A, B> {
    pub account_or_mint: &'a mut A,
    pub current_authority: &'a B,
}
