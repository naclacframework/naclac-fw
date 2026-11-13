//! The `TokenCpi` trait: the token instructions as methods on the token
//! program account types, taking `*Accounts` bundles.

use {
    super::*,
    crate::{
        prelude::{Address, CpiSigner, Result},
        wrappers::{
            Interface, Program, ToCpiHandle, ToCpiHandleMut, Token, Token2022, TokenInterface,
        },
    },
};

pub trait TokenCpi {
    fn transfer_checked<'a, A, B, C, D>(
        &self,
        accounts: TransferCheckedAccounts<'a, A, B, C, D>,
        amount: u64,
        decimals: u8,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandleMut<'a>,
        D: ToCpiHandle<'a>;

    fn transfer_checked_signed<'a, A, B, C, D>(
        &self,
        accounts: TransferCheckedAccounts<'a, A, B, C, D>,
        amount: u64,
        decimals: u8,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandleMut<'a>,
        D: ToCpiHandle<'a>;

    fn mint_to<'a, A, B, C>(
        &self,
        accounts: MintToAccounts<'a, A, B, C>,
        amount: u64,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn mint_to_signed<'a, A, B, C>(
        &self,
        accounts: MintToAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn burn<'a, A, B, C>(&self, accounts: BurnAccounts<'a, A, B, C>, amount: u64) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn burn_signed<'a, A, B, C>(
        &self,
        accounts: BurnAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn close_account<'a, A, B, C>(&self, accounts: CloseAccountAccounts<'a, A, B, C>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    fn close_account_signed<'a, A, B, C>(
        &self,
        accounts: CloseAccountAccounts<'a, A, B, C>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>;

    /// Permissionless (no authority to sign for) — no `_signed` counterpart.
    fn sync_native<'a, A>(&self, accounts: SyncNativeAccounts<'a, A>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn approve<'a, A, B, C>(
        &self,
        accounts: ApproveAccounts<'a, A, B, C>,
        amount: u64,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn approve_signed<'a, A, B, C>(
        &self,
        accounts: ApproveAccounts<'a, A, B, C>,
        amount: u64,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn revoke<'a, A, B>(&self, accounts: RevokeAccounts<'a, A, B>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn revoke_signed<'a, A, B>(
        &self,
        accounts: RevokeAccounts<'a, A, B>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn initialize_mint<'a, A>(
        &self,
        accounts: InitializeMintAccounts<'a, A>,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn initialize_mint_signed<'a, A>(
        &self,
        accounts: InitializeMintAccounts<'a, A>,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>;

    fn initialize_account<'a, A, B, C>(
        &self,
        accounts: InitializeAccountAccounts<'a, A, B, C>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn initialize_account_signed<'a, A, B, C>(
        &self,
        accounts: InitializeAccountAccounts<'a, A, B, C>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn freeze_account<'a, A, B, C>(
        &self,
        accounts: FreezeAccountAccounts<'a, A, B, C>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn freeze_account_signed<'a, A, B, C>(
        &self,
        accounts: FreezeAccountAccounts<'a, A, B, C>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn thaw_account<'a, A, B, C>(&self, accounts: ThawAccountAccounts<'a, A, B, C>) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn thaw_account_signed<'a, A, B, C>(
        &self,
        accounts: ThawAccountAccounts<'a, A, B, C>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>,
        C: ToCpiHandle<'a>;

    fn set_authority<'a, A, B>(
        &self,
        accounts: SetAuthorityAccounts<'a, A, B>,
        authority_type: AuthorityType,
        new_authority: Option<&Address>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;

    fn set_authority_signed<'a, A, B>(
        &self,
        accounts: SetAuthorityAccounts<'a, A, B>,
        authority_type: AuthorityType,
        new_authority: Option<&Address>,
        signer_seeds: &[CpiSigner<'_, '_>],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandle<'a>;
}

macro_rules! impl_token_cpi_helpers {
    ($type:ty) => {
        impl TokenCpi for $type {
            fn transfer_checked<'a, A, B, C, D>(
                &self,
                accounts: TransferCheckedAccounts<'a, A, B, C, D>,
                amount: u64,
                decimals: u8,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandleMut<'a>,
                D: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer_checked(
                    program_h,
                    from_h,
                    mint_h,
                    to_h,
                    authority_h,
                    amount,
                    decimals,
                )
            }

            fn transfer_checked_signed<'a, A, B, C, D>(
                &self,
                accounts: TransferCheckedAccounts<'a, A, B, C, D>,
                amount: u64,
                decimals: u8,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandleMut<'a>,
                D: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let from_h = accounts.from.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                transfer_checked_signed(
                    program_h,
                    from_h,
                    mint_h,
                    to_h,
                    authority_h,
                    CheckedTransferParams { amount, decimals },
                    signer_seeds,
                )
            }

            fn mint_to<'a, A, B, C>(
                &self,
                accounts: MintToAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                mint_to(program_h, mint_h, to_h, authority_h, amount)
            }

            fn mint_to_signed<'a, A, B, C>(
                &self,
                accounts: MintToAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let to_h = accounts.to.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                mint_to_signed(program_h, mint_h, to_h, authority_h, amount, signer_seeds)
            }

            fn burn<'a, A, B, C>(
                &self,
                accounts: BurnAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let from_h = accounts.from.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                burn(program_h, mint_h, from_h, authority_h, amount)
            }

            fn burn_signed<'a, A, B, C>(
                &self,
                accounts: BurnAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                let from_h = accounts.from.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                burn_signed(program_h, mint_h, from_h, authority_h, amount, signer_seeds)
            }

            fn close_account<'a, A, B, C>(
                &self,
                accounts: CloseAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let destination_h = accounts.destination.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                close_account(program_h, account_h, destination_h, authority_h)
            }

            fn close_account_signed<'a, A, B, C>(
                &self,
                accounts: CloseAccountAccounts<'a, A, B, C>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandleMut<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let destination_h = accounts.destination.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                close_account_signed(
                    program_h,
                    account_h,
                    destination_h,
                    authority_h,
                    signer_seeds,
                )
            }

            fn sync_native<'a, A>(&self, accounts: SyncNativeAccounts<'a, A>) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let native_token_h = accounts.native_token.to_cpi_handle_mut();
                sync_native(program_h, native_token_h)
            }

            fn initialize_mint<'a, A>(
                &self,
                accounts: InitializeMintAccounts<'a, A>,
                decimals: u8,
                mint_authority: &Address,
                freeze_authority: Option<&Address>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                initialize_mint(
                    program_h,
                    mint_h,
                    decimals,
                    mint_authority,
                    freeze_authority,
                )
            }

            fn initialize_mint_signed<'a, A>(
                &self,
                accounts: InitializeMintAccounts<'a, A>,
                decimals: u8,
                mint_authority: &Address,
                freeze_authority: Option<&Address>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
            {
                let program_h = self.to_cpi_handle();
                let mint_h = accounts.mint.to_cpi_handle_mut();
                initialize_mint_signed(
                    program_h,
                    mint_h,
                    decimals,
                    mint_authority,
                    freeze_authority,
                    signer_seeds,
                )
            }

            fn approve<'a, A, B, C>(
                &self,
                accounts: ApproveAccounts<'a, A, B, C>,
                amount: u64,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let delegate_h = accounts.delegate.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                approve(program_h, to_h, delegate_h, authority_h, amount)
            }

            fn approve_signed<'a, A, B, C>(
                &self,
                accounts: ApproveAccounts<'a, A, B, C>,
                amount: u64,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let to_h = accounts.to.to_cpi_handle_mut();
                let delegate_h = accounts.delegate.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                approve_signed(
                    program_h,
                    to_h,
                    delegate_h,
                    authority_h,
                    amount,
                    signer_seeds,
                )
            }

            fn revoke<'a, A, B>(&self, accounts: RevokeAccounts<'a, A, B>) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let source_h = accounts.source.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                revoke(program_h, source_h, authority_h)
            }

            fn revoke_signed<'a, A, B>(
                &self,
                accounts: RevokeAccounts<'a, A, B>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let source_h = accounts.source.to_cpi_handle_mut();
                let authority_h = accounts.authority.to_cpi_handle();
                revoke_signed(program_h, source_h, authority_h, signer_seeds)
            }

            fn initialize_account<'a, A, B, C>(
                &self,
                accounts: InitializeAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                initialize_account(program_h, account_h, mint_h, authority_h)
            }

            fn initialize_account_signed<'a, A, B, C>(
                &self,
                accounts: InitializeAccountAccounts<'a, A, B, C>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                initialize_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn freeze_account<'a, A, B, C>(
                &self,
                accounts: FreezeAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                freeze_account(program_h, account_h, mint_h, authority_h)
            }

            fn freeze_account_signed<'a, A, B, C>(
                &self,
                accounts: FreezeAccountAccounts<'a, A, B, C>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                freeze_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn thaw_account<'a, A, B, C>(
                &self,
                accounts: ThawAccountAccounts<'a, A, B, C>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                thaw_account(program_h, account_h, mint_h, authority_h)
            }

            fn thaw_account_signed<'a, A, B, C>(
                &self,
                accounts: ThawAccountAccounts<'a, A, B, C>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
                C: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_h = accounts.account.to_cpi_handle_mut();
                let mint_h = accounts.mint.to_cpi_handle();
                let authority_h = accounts.authority.to_cpi_handle();
                thaw_account_signed(program_h, account_h, mint_h, authority_h, signer_seeds)
            }

            fn set_authority<'a, A, B>(
                &self,
                accounts: SetAuthorityAccounts<'a, A, B>,
                authority_type: AuthorityType,
                new_authority: Option<&Address>,
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_or_mint_h = accounts.account_or_mint.to_cpi_handle_mut();
                let current_authority_h = accounts.current_authority.to_cpi_handle();
                set_authority(
                    program_h,
                    account_or_mint_h,
                    current_authority_h,
                    authority_type,
                    new_authority,
                )
            }

            fn set_authority_signed<'a, A, B>(
                &self,
                accounts: SetAuthorityAccounts<'a, A, B>,
                authority_type: AuthorityType,
                new_authority: Option<&Address>,
                signer_seeds: &[CpiSigner<'_, '_>],
            ) -> Result<()>
            where
                A: ToCpiHandleMut<'a>,
                B: ToCpiHandle<'a>,
            {
                let program_h = self.to_cpi_handle();
                let account_or_mint_h = accounts.account_or_mint.to_cpi_handle_mut();
                let current_authority_h = accounts.current_authority.to_cpi_handle();
                set_authority_signed(
                    program_h,
                    account_or_mint_h,
                    current_authority_h,
                    authority_type,
                    new_authority,
                    signer_seeds,
                )
            }
        }
    };
}

impl_token_cpi_helpers!(Program<Token>);
impl_token_cpi_helpers!(Program<Token2022>);
impl_token_cpi_helpers!(Interface<TokenInterface>);
