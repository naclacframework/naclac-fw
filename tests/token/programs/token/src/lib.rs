#![no_std]
use naclac_lang::prelude::*;

declare_id!("ExLoWWAhvnCCaLd59rMMUrqZzqgMiNcRWLJ5YuaWEQ28");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod token {
    pub fn init_mint_authority(ctx: Context<InitMintAuthority>) -> Result {
        init_mint_authority::init_mint_authority(ctx)
    }

    pub fn create_mint(
        ctx: Context<CreateMint>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint::create_mint(ctx, id, mint_bump, decimals)
    }

    pub fn create_mint_with_freeze(
        ctx: Context<CreateMintWithFreeze>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint_with_freeze::create_mint_with_freeze(ctx, id, mint_bump, decimals)
    }

    pub fn check_vault_constraints(ctx: Context<CheckVaultConstraints>) -> Result {
        check_vault_constraints::check_vault_constraints(ctx)
    }

    pub fn check_vault_program(ctx: Context<CheckVaultProgram>) -> Result {
        check_vault_program::check_vault_program(ctx)
    }

    pub fn check_mint_freeze_authority(ctx: Context<CheckMintFreezeAuthority>) -> Result {
        check_mint_freeze_authority::check_mint_freeze_authority(ctx)
    }

    pub fn mint_to_vault(ctx: Context<MintToVault>, amount: u64) -> Result {
        mint_to_vault::mint_to_vault(ctx, amount)
    }

    pub fn transfer_tokens(ctx: Context<TransferTokens>, amount: u64) -> Result {
        transfer_tokens::transfer_tokens(ctx, amount)
    }

    pub fn create_ata(ctx: Context<CreateAssociatedTokenAccount>) -> Result {
        create_ata::create_ata(ctx)
    }

    pub fn create_ata_idempotent(ctx: Context<CreateAssociatedTokenAccountIdempotent>) -> Result {
        create_ata_idempotent::create_ata_idempotent(ctx)
    }

    pub fn check_ata_constraints(ctx: Context<CheckAtaConstraints>, ata_bump: u8) -> Result {
        check_ata_constraints::check_ata_constraints(ctx, ata_bump)
    }

    pub fn burn_vault_tokens(ctx: Context<BurnVaultTokens>, amount: u64) -> Result {
        burn_vault_tokens::burn_vault_tokens(ctx, amount)
    }

    pub fn set_mint_authority(ctx: Context<SetMintAuthority>, authority_type: u8) -> Result {
        set_mint_authority::set_mint_authority(ctx, authority_type)
    }

    pub fn freeze_vault_account(ctx: Context<FreezeVaultAccount>) -> Result {
        freeze_vault_account::freeze_vault_account(ctx)
    }

    pub fn thaw_vault_account(ctx: Context<ThawVaultAccount>) -> Result {
        thaw_vault_account::thaw_vault_account(ctx)
    }

    pub fn approve_vault_delegate(ctx: Context<ApproveVaultDelegate>, amount: u64) -> Result {
        approve_vault_delegate::approve_vault_delegate(ctx, amount)
    }

    pub fn revoke_vault_delegate(ctx: Context<RevokeVaultDelegate>) -> Result {
        revoke_vault_delegate::revoke_vault_delegate(ctx)
    }

    pub fn close_vault_account(ctx: Context<CloseVaultAccount>) -> Result {
        close_vault_account::close_vault_account(ctx)
    }

    pub fn transfer_tokens_checked(
        ctx: Context<TransferTokensChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result {
        transfer_tokens_checked::transfer_tokens_checked(ctx, amount, decimals)
    }

    pub fn create_mint2022(
        ctx: Context<CreateMint2022>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022::create_mint2022(ctx, id, mint_bump, decimals)
    }

    pub fn mint_to_vault2022(ctx: Context<MintToVault2022>, amount: u64) -> Result {
        mint_to_vault2022::mint_to_vault2022(ctx, amount)
    }

    pub fn create_mint2022_dual_token_program(
        ctx: Context<CreateMint2022DualTokenProgram>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022_dual_token_program::create_mint2022_dual_token_program(ctx, id, mint_bump, decimals)
    }

    pub fn transfer_tokens2022(ctx: Context<TransferTokens2022>, amount: u64) -> Result {
        transfer_tokens2022::transfer_tokens2022(ctx, amount)
    }

    pub fn check_transfer_fee_config(
        ctx: Context<CheckTransferFeeConfig>,
        args: CheckTransferFeeConfigArgs,
    ) -> Result {
        check_transfer_fee_config::check_transfer_fee_config(ctx, args)
    }

    pub fn create_mint2022_with_transfer_fee(
        ctx: Context<CreateMint2022WithTransferFee>,
        args: CreateMint2022WithTransferFeeArgs,
    ) -> Result {
        create_mint2022_with_transfer_fee::create_mint2022_with_transfer_fee(ctx, args)
    }

    pub fn create_mint2022_with_transfer_hook(
        ctx: Context<CreateMint2022WithTransferHook>,
        args: CreateMint2022WithTransferHookArgs,
    ) -> Result {
        create_mint2022_with_transfer_hook::create_mint2022_with_transfer_hook(ctx, args)
    }

    pub fn check_account_fields(
        ctx: Context<CheckAccountFields>,
        args: CheckAccountFieldsArgs,
    ) -> Result {
        check_account_fields::check_account_fields(ctx, args)
    }

    pub fn check_transfer_hook(
        ctx: Context<CheckTransferHook>,
        expected_authority: Option<Address>,
        expected_program_id: Option<Address>,
        expected_transferring: u8,
    ) -> Result {
        check_transfer_hook::check_transfer_hook(
            ctx,
            expected_authority,
            expected_program_id,
            expected_transferring,
        )
    }

    pub fn check_permanent_delegate(
        ctx: Context<CheckPermanentDelegate>,
        expected_delegate: Option<Address>,
    ) -> Result {
        check_permanent_delegate::check_permanent_delegate(ctx, expected_delegate)
    }

    pub fn exercise_transfer_fee_cpis(
        ctx: Context<ExerciseTransferFeeCpis>,
        args: ExerciseTransferFeeCpisArgs,
    ) -> Result {
        exercise_transfer_fee_cpis::exercise_transfer_fee_cpis(ctx, args)
    }

    pub fn exercise_transfer_hook_update(
        ctx: Context<ExerciseTransferHookUpdate>,
        new_hook_program_id: Address,
    ) -> Result {
        exercise_transfer_hook_update::exercise_transfer_hook_update(ctx, new_hook_program_id)
    }

    pub fn create_mint2022_with_default_account_state(
        ctx: Context<CreateMint2022WithDefaultAccountState>,
        args: CreateMint2022WithDefaultAccountStateArgs,
    ) -> Result {
        create_mint2022_with_default_account_state::create_mint2022_with_default_account_state(ctx, args)
    }

    pub fn check_default_account_state(
        ctx: Context<CheckDefaultAccountState>,
        expected_state: u8,
    ) -> Result {
        check_default_account_state::check_default_account_state(ctx, expected_state)
    }

    pub fn exercise_default_account_state_update(
        ctx: Context<ExerciseDefaultAccountStateUpdate>,
        new_state: u8,
    ) -> Result {
        exercise_default_account_state_update::exercise_default_account_state_update(ctx, new_state)
    }

    pub fn create_token_account_with_immutable_owner(
        ctx: Context<CreateTokenAccountWithImmutableOwner>,
    ) -> Result {
        create_token_account_with_immutable_owner::create_token_account_with_immutable_owner(ctx)
    }

    pub fn check_immutable_owner(
        ctx: Context<CheckImmutableOwner>,
        expected_present: u8,
    ) -> Result {
        check_immutable_owner::check_immutable_owner(ctx, expected_present)
    }

    pub fn set_token_account_owner(ctx: Context<SetTokenAccountOwner>) -> Result {
        set_token_account_owner::set_token_account_owner(ctx)
    }

    pub fn create_mint2022_with_interest_bearing_mint(
        ctx: Context<CreateMint2022WithInterestBearingMint>,
        args: CreateMint2022WithInterestBearingMintArgs,
    ) -> Result {
        create_mint2022_with_interest_bearing_mint::create_mint2022_with_interest_bearing_mint(ctx, args)
    }

    pub fn check_interest_bearing_mint(
        ctx: Context<CheckInterestBearingMint>,
        args: CheckInterestBearingMintArgs,
    ) -> Result {
        check_interest_bearing_mint::check_interest_bearing_mint(ctx, args)
    }

    pub fn exercise_interest_bearing_mint_update_rate(
        ctx: Context<ExerciseInterestBearingMintUpdateRate>,
        new_rate: i16,
    ) -> Result {
        exercise_interest_bearing_mint_update_rate::exercise_interest_bearing_mint_update_rate(ctx, new_rate)
    }

    pub fn create_mint2022_with_metadata_pointer(
        ctx: Context<CreateMint2022WithMetadataPointer>,
        args: CreateMint2022WithMetadataPointerArgs,
    ) -> Result {
        create_mint2022_with_metadata_pointer::create_mint2022_with_metadata_pointer(ctx, args)
    }

    pub fn check_metadata_pointer(
        ctx: Context<CheckMetadataPointer>,
        expected_authority: Option<Address>,
        expected_metadata_address: Option<Address>,
    ) -> Result {
        check_metadata_pointer::check_metadata_pointer(ctx, expected_authority, expected_metadata_address)
    }

    pub fn exercise_metadata_pointer_update(
        ctx: Context<ExerciseMetadataPointerUpdate>,
        new_metadata_address: Address,
    ) -> Result {
        exercise_metadata_pointer_update::exercise_metadata_pointer_update(ctx, new_metadata_address)
    }

    pub fn create_mint2022_with_group_pointer(
        ctx: Context<CreateMint2022WithGroupPointer>,
        args: CreateMint2022WithGroupPointerArgs,
    ) -> Result {
        create_mint2022_with_group_pointer::create_mint2022_with_group_pointer(ctx, args)
    }

    pub fn check_group_pointer(
        ctx: Context<CheckGroupPointer>,
        expected_authority: Option<Address>,
        expected_group_address: Option<Address>,
    ) -> Result {
        check_group_pointer::check_group_pointer(ctx, expected_authority, expected_group_address)
    }

    pub fn exercise_group_pointer_update(
        ctx: Context<ExerciseGroupPointerUpdate>,
        new_group_address: Address,
    ) -> Result {
        exercise_group_pointer_update::exercise_group_pointer_update(ctx, new_group_address)
    }

    pub fn create_mint2022_with_group_member_pointer(
        ctx: Context<CreateMint2022WithGroupMemberPointer>,
        args: CreateMint2022WithGroupMemberPointerArgs,
    ) -> Result {
        create_mint2022_with_group_member_pointer::create_mint2022_with_group_member_pointer(ctx, args)
    }

    pub fn check_group_member_pointer(
        ctx: Context<CheckGroupMemberPointer>,
        expected_authority: Option<Address>,
        expected_member_address: Option<Address>,
    ) -> Result {
        check_group_member_pointer::check_group_member_pointer(ctx, expected_authority, expected_member_address)
    }

    pub fn exercise_group_member_pointer_update(
        ctx: Context<ExerciseGroupMemberPointerUpdate>,
        new_member_address: Address,
    ) -> Result {
        exercise_group_member_pointer_update::exercise_group_member_pointer_update(ctx, new_member_address)
    }

    pub fn create_mint2022_with_permanent_delegate(
        ctx: Context<CreateMint2022WithPermanentDelegate>,
        args: CreateMint2022WithPermanentDelegateArgs,
    ) -> Result {
        create_mint2022_with_permanent_delegate::create_mint2022_with_permanent_delegate(ctx, args)
    }

    pub fn exercise_permanent_delegate_burn(
        ctx: Context<ExercisePermanentDelegateBurn>,
        amount: u64,
    ) -> Result {
        exercise_permanent_delegate_burn::exercise_permanent_delegate_burn(ctx, amount)
    }

    pub fn create_mint2022_with_non_transferable(
        ctx: Context<CreateMint2022WithNonTransferable>,
        args: CreateMint2022WithNonTransferableArgs,
    ) -> Result {
        create_mint2022_with_non_transferable::create_mint2022_with_non_transferable(ctx, args)
    }

    pub fn check_non_transferable(
        ctx: Context<CheckNonTransferable>,
        expected_present: u8,
    ) -> Result {
        check_non_transferable::check_non_transferable(ctx, expected_present)
    }

    pub fn create_mint2022_with_metadata_pointer_and_metadata(
        ctx: Context<CreateMint2022WithMetadataPointerAndMetadata>,
        args: CreateMint2022WithMetadataPointerAndMetadataArgs,
    ) -> Result {
        create_mint2022_with_metadata_pointer_and_metadata::create_mint2022_with_metadata_pointer_and_metadata(ctx, args)
    }

    pub fn exercise_token_metadata_lifecycle(
        ctx: Context<ExerciseTokenMetadataLifecycle>,
        new_name: ZcString,
        extra_key: ZcString,
        extra_value: ZcString,
    ) -> Result {
        exercise_token_metadata_lifecycle::exercise_token_metadata_lifecycle(ctx, new_name, extra_key, extra_value)
    }

    pub fn exercise_token_metadata_remove_key_and_authority(
        ctx: Context<ExerciseTokenMetadataRemoveKeyAndAuthority>,
        key_to_remove: ZcString,
        new_authority: Address,
    ) -> Result {
        exercise_token_metadata_remove_key_and_authority::exercise_token_metadata_remove_key_and_authority(ctx, key_to_remove, new_authority)
    }

    pub fn create_mint2022_with_group_pointer_and_group(
        ctx: Context<CreateMint2022WithGroupPointerAndGroup>,
        args: CreateMint2022WithGroupPointerAndGroupArgs,
    ) -> Result {
        create_mint2022_with_group_pointer_and_group::create_mint2022_with_group_pointer_and_group(ctx, args)
    }

    pub fn check_token_group(
        ctx: Context<CheckTokenGroup>,
        expected_size: u64,
        expected_max_size: u64,
    ) -> Result {
        check_token_group::check_token_group(ctx, expected_size, expected_max_size)
    }

    pub fn exercise_update_token_group(
        ctx: Context<ExerciseUpdateTokenGroup>,
        new_max_size: u64,
        new_authority: Address,
    ) -> Result {
        exercise_update_token_group::exercise_update_token_group(ctx, new_max_size, new_authority)
    }

    pub fn create_mint2022_with_group_member_pointer_and_member(
        ctx: Context<CreateMint2022WithGroupMemberPointerAndMember>,
        args: CreateMint2022WithGroupMemberPointerAndMemberArgs,
    ) -> Result {
        create_mint2022_with_group_member_pointer_and_member::create_mint2022_with_group_member_pointer_and_member(ctx, args)
    }

    pub fn check_token_group_member(
        ctx: Context<CheckTokenGroupMember>,
        expected_mint: Address,
        expected_group: Address,
    ) -> Result {
        check_token_group_member::check_token_group_member(ctx, expected_mint, expected_group)
    }

    pub fn create_token_account_with_memo_transfer_required(
        ctx: Context<CreateTokenAccountWithMemoTransferRequired>,
    ) -> Result {
        create_token_account_with_memo_transfer_required::create_token_account_with_memo_transfer_required(ctx)
    }

    pub fn check_memo_transfer(
        ctx: Context<CheckMemoTransfer>,
        expected_present: u8,
        expected_required: u8,
    ) -> Result {
        check_memo_transfer::check_memo_transfer(ctx, expected_present, expected_required)
    }

    pub fn exercise_memo_transfer_disable(ctx: Context<ExerciseMemoTransferDisable>) -> Result {
        exercise_memo_transfer_disable::exercise_memo_transfer_disable(ctx)
    }

    pub fn transfer_tokens2022_with_memo(
        ctx: Context<TransferTokens2022WithMemo>,
        amount: u64,
    ) -> Result {
        transfer_tokens2022_with_memo::transfer_tokens2022_with_memo(ctx, amount)
    }

    pub fn check_cpi_guard(
        ctx: Context<CheckCpiGuard>,
        expected_present: u8,
        expected_locked: u8,
    ) -> Result {
        check_cpi_guard::check_cpi_guard(ctx, expected_present, expected_locked)
    }

    pub fn approve_vault_delegate2022(
        ctx: Context<ApproveVaultDelegate2022>,
        amount: u64,
    ) -> Result {
        approve_vault_delegate2022::approve_vault_delegate2022(ctx, amount)
    }

    pub fn create_mint2022_with_pausable(
        ctx: Context<CreateMint2022WithPausable>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022_with_pausable::create_mint2022_with_pausable(ctx, id, mint_bump, decimals)
    }

    pub fn check_pausable_config(
        ctx: Context<CheckPausableConfig>,
        expected_authority: Option<Address>,
        expected_paused: u8,
    ) -> Result {
        check_pausable_config::check_pausable_config(ctx, expected_authority, expected_paused)
    }

    pub fn exercise_pause_mint(ctx: Context<ExercisePauseMint>) -> Result {
        exercise_pause_mint::exercise_pause_mint(ctx)
    }

    pub fn exercise_resume_mint(ctx: Context<ExerciseResumeMint>) -> Result {
        exercise_resume_mint::exercise_resume_mint(ctx)
    }

    pub fn create_mint2022_with_mint_close_authority(
        ctx: Context<CreateMint2022WithMintCloseAuthority>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022_with_mint_close_authority::create_mint2022_with_mint_close_authority(
            ctx, id, mint_bump, decimals,
        )
    }

    pub fn check_mint_close_authority(
        ctx: Context<CheckMintCloseAuthority>,
        expected_close_authority: Option<Address>,
    ) -> Result {
        check_mint_close_authority::check_mint_close_authority(ctx, expected_close_authority)
    }

    pub fn close_mint2022(ctx: Context<CloseMint2022>) -> Result {
        close_mint2022::close_mint2022(ctx)
    }

    pub fn create_mint2022_with_scaled_ui_amount(
        ctx: Context<CreateMint2022WithScaledUiAmount>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
        multiplier_bits: u64,
    ) -> Result {
        create_mint2022_with_scaled_ui_amount::create_mint2022_with_scaled_ui_amount(
            ctx, id, mint_bump, decimals, multiplier_bits,
        )
    }

    pub fn check_scaled_ui_amount_config(
        ctx: Context<CheckScaledUiAmountConfig>,
        expected_authority: Option<Address>,
        expected_multiplier_bits: u64,
    ) -> Result {
        check_scaled_ui_amount_config::check_scaled_ui_amount_config(
            ctx, expected_authority, expected_multiplier_bits,
        )
    }

    pub fn exercise_update_scaled_ui_amount_multiplier(
        ctx: Context<ExerciseUpdateScaledUiAmountMultiplier>,
        new_multiplier_bits: u64,
        effective_timestamp: i64,
    ) -> Result {
        exercise_update_scaled_ui_amount_multiplier::exercise_update_scaled_ui_amount_multiplier(
            ctx, new_multiplier_bits, effective_timestamp,
        )
    }

    pub fn create_mint2022_with_permissioned_burn(
        ctx: Context<CreateMint2022WithPermissionedBurn>,
        id: u64,
        mint_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint2022_with_permissioned_burn::create_mint2022_with_permissioned_burn(
            ctx, id, mint_bump, decimals,
        )
    }

    pub fn check_permissioned_burn(
        ctx: Context<CheckPermissionedBurn>,
        expected_authority: Option<Address>,
    ) -> Result {
        check_permissioned_burn::check_permissioned_burn(ctx, expected_authority)
    }

    pub fn burn_vault_tokens2022(ctx: Context<BurnVaultTokens2022>, amount: u64) -> Result {
        burn_vault_tokens2022::burn_vault_tokens2022(ctx, amount)
    }

    pub fn exercise_permissioned_burn(
        ctx: Context<ExercisePermissionedBurn>,
        amount: u64,
    ) -> Result {
        exercise_permissioned_burn::exercise_permissioned_burn(ctx, amount)
    }
}
