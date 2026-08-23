#![cfg_attr(feature = "pinocchio", no_std)]

pub(crate) use naclac_core::wrappers;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub(crate) use naclac_core::cpi;

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub(crate) use naclac_core::borsh;

#[cfg(feature = "pinocchio")]
extern crate alloc;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod token;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod associated_token;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod extensions;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub use token::*;

pub mod prelude {
    pub use naclac_core::prelude::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::token::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::associated_token::{AssociatedTokenCpi, CreateAtaAccounts};

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::extensions::{
        disable_required_memo_transfers, disable_required_memo_transfers_signed,
        enable_required_memo_transfers, enable_required_memo_transfers_signed,
        harvest_withheld_tokens_to_mint, harvest_withheld_tokens_to_mint_signed,
        initialize_default_account_state, initialize_default_account_state_signed,
        initialize_group_member_pointer, initialize_group_member_pointer_signed,
        initialize_group_pointer, initialize_group_pointer_signed, initialize_immutable_owner,
        initialize_immutable_owner_signed, initialize_interest_bearing_mint,
        initialize_interest_bearing_mint_signed, initialize_metadata_pointer,
        initialize_metadata_pointer_signed, initialize_mint_close_authority,
        initialize_mint_close_authority_signed, initialize_non_transferable_mint,
        initialize_non_transferable_mint_signed, initialize_pausable_config,
        initialize_pausable_config_signed, initialize_permanent_delegate,
        initialize_permanent_delegate_signed, initialize_permissioned_burn,
        initialize_permissioned_burn_signed, initialize_scaled_ui_amount_config,
        initialize_scaled_ui_amount_config_signed, initialize_transfer_fee_config,
        initialize_transfer_fee_config_signed, initialize_transfer_hook,
        initialize_transfer_hook_signed, pause_mint, pause_mint_signed,
        permissioned_burn, permissioned_burn_checked, permissioned_burn_checked_signed,
        permissioned_burn_signed, resume_mint,
        resume_mint_signed, set_transfer_fee, set_transfer_fee_signed,
        transfer_checked_with_fee, transfer_checked_with_fee_signed, transfer_hook_update,
        transfer_hook_update_signed, update_default_account_state,
        update_default_account_state_signed, update_group_member_pointer,
        update_group_member_pointer_signed, update_group_pointer, update_group_pointer_signed,
        update_interest_bearing_mint_rate, update_interest_bearing_mint_rate_signed,
        update_metadata_pointer, update_metadata_pointer_signed,
        update_scaled_ui_amount_multiplier, update_scaled_ui_amount_multiplier_signed,
        withdraw_withheld_tokens_from_accounts, withdraw_withheld_tokens_from_accounts_signed,
        withdraw_withheld_tokens_from_mint, withdraw_withheld_tokens_from_mint_signed,
        CpiGuard, DefaultAccountState, Extension, GroupMemberPointer, GroupPointer, ImmutableOwner,
        InterestBearingConfig, MemoTransfer, MetadataPointer, MintCloseAuthority,
        NonTransferable,
        NonTransferableAccount, PausableAccount, PausableConfig, PermanentDelegate,
        PermissionedBurnConfig, ScaledUiAmountConfig,
        TokenInterfaceAccountExtensions,
        TransferFeeAmount, TransferFeeConfig, TransferHook, TransferHookAccount,
        MAX_TRANSFER_FEE_SOURCE_ACCOUNTS,
    };

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::extensions::{
        emit_token_metadata, initialize_token_group, initialize_token_group_member,
        initialize_token_group_member_signed, initialize_token_group_signed,
        initialize_token_metadata, initialize_token_metadata_signed, remove_token_metadata_key,
        remove_token_metadata_key_signed, update_token_group_authority,
        update_token_group_authority_signed, update_token_group_max_size,
        update_token_group_max_size_signed, update_token_metadata_authority,
        update_token_metadata_authority_signed, update_token_metadata_field,
        update_token_metadata_field_signed, Field, TokenGroupMemberInitializeAccounts,
        TokenMetadataInitializeParams,
    };

    #[cfg(all(feature = "solana", not(feature = "pinocchio")))]
    pub use crate::extensions::{transfer_checked_with_hook, transfer_checked_with_hook_signed};

    #[cfg(feature = "pinocchio")]
    pub use crate::{
        pinocchio_associated_token_account, pinocchio_token, pinocchio_token_2022,
        spl_token_group_interface, spl_token_metadata_interface,
    };

    #[cfg(all(feature = "solana", not(feature = "pinocchio")))]
    pub use crate::{
        spl_associated_token_account, spl_token, spl_token_2022, spl_token_group_interface,
        spl_token_metadata_interface,
    };
}

#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_associated_token_account;
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_token;
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio_token_2022;

#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_associated_token_account;
#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_token;
#[cfg(all(feature = "solana", not(feature = "pinocchio")))]
pub extern crate spl_token_2022;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub extern crate spl_token_group_interface;
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub extern crate spl_token_metadata_interface;
