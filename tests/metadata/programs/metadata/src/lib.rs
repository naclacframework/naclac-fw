#![no_std]
use naclac_lang::prelude::*;

declare_id!("CfCmyXnF1To9ryX2g5Sj3X8yhpFopYxZFrgFf8fWVwU5");

pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod metadata {
    pub fn create_asset(ctx: Context<CreateAsset>, name: ZcString, uri: ZcString) -> Result {
        create_asset::create_asset(ctx, name, uri)
    }

    pub fn create_collection(
        ctx: Context<CreateCollection>,
        name: ZcString,
        uri: ZcString,
    ) -> Result {
        create_collection::create_collection(ctx, name, uri)
    }

    pub fn attach_royalties(ctx: Context<AttachRoyalties>, basis_points: u16) -> Result {
        attach_royalties::attach_royalties(ctx, basis_points)
    }

    pub fn attach_attributes(ctx: Context<AttachAttributes>, key: ZcString, value: ZcString) -> Result {
        attach_attributes::attach_attributes(ctx, key, value)
    }

    pub fn attach_transfer_delegate(ctx: Context<AttachTransferDelegate>) -> Result {
        attach_transfer_delegate::attach_transfer_delegate(ctx)
    }

    pub fn attach_burn_delegate(ctx: Context<AttachBurnDelegate>) -> Result {
        attach_burn_delegate::attach_burn_delegate(ctx)
    }

    pub fn attach_permanent_transfer_delegate(
        ctx: Context<AttachPermanentTransferDelegate>,
    ) -> Result {
        attach_permanent_transfer_delegate::attach_permanent_transfer_delegate(ctx)
    }

    pub fn attach_permanent_burn_delegate(ctx: Context<AttachPermanentBurnDelegate>) -> Result {
        attach_permanent_burn_delegate::attach_permanent_burn_delegate(ctx)
    }

    pub fn attach_immutable_metadata(ctx: Context<AttachImmutableMetadata>) -> Result {
        attach_immutable_metadata::attach_immutable_metadata(ctx)
    }

    pub fn attach_master_edition(ctx: Context<AttachMasterEdition>) -> Result {
        attach_master_edition::attach_master_edition(ctx)
    }

    pub fn attach_verified_creators(ctx: Context<AttachVerifiedCreators>) -> Result {
        attach_verified_creators::attach_verified_creators(ctx)
    }

    pub fn attach_app_data(ctx: Context<AttachAppData>) -> Result {
        attach_app_data::attach_app_data(ctx)
    }

    pub fn attach_oracle(ctx: Context<AttachOracle>) -> Result {
        attach_oracle::attach_oracle(ctx)
    }

    pub fn attach_update_delegate(ctx: Context<AttachUpdateDelegate>) -> Result {
        attach_update_delegate::attach_update_delegate(ctx)
    }

    pub fn attach_autograph(ctx: Context<AttachAutograph>) -> Result {
        attach_autograph::attach_autograph(ctx)
    }

    pub fn attach_freeze_delegate(ctx: Context<AttachFreezeDelegate>) -> Result {
        attach_freeze_delegate::attach_freeze_delegate(ctx)
    }

    pub fn transfer_asset(ctx: Context<TransferAsset>) -> Result {
        transfer_asset::transfer_asset(ctx)
    }

    pub fn burn_asset(ctx: Context<BurnAsset>) -> Result {
        burn_asset::burn_asset(ctx)
    }

    pub fn update_asset(ctx: Context<UpdateAsset>, new_name: ZcString, new_uri: ZcString) -> Result {
        update_asset::update_asset(ctx, new_name, new_uri)
    }

    pub fn update_collection(
        ctx: Context<UpdateCollection>,
        new_name: ZcString,
        new_uri: ZcString,
    ) -> Result {
        update_collection::update_collection(ctx, new_name, new_uri)
    }

    pub fn burn_collection(ctx: Context<BurnCollection>) -> Result {
        burn_collection::burn_collection(ctx)
    }

    pub fn remove_freeze_delegate(ctx: Context<RemoveFreezeDelegate>) -> Result {
        remove_freeze_delegate::remove_freeze_delegate(ctx)
    }

    pub fn update_freeze_delegate(ctx: Context<UpdateFreezeDelegate>) -> Result {
        update_freeze_delegate::update_freeze_delegate(ctx)
    }

    pub fn approve_plugin_authority(ctx: Context<ApprovePluginAuthority>) -> Result {
        approve_plugin_authority::approve_plugin_authority(ctx)
    }

    pub fn revoke_plugin_authority(ctx: Context<RevokePluginAuthority>) -> Result {
        revoke_plugin_authority::revoke_plugin_authority(ctx)
    }

    pub fn remove_master_edition(ctx: Context<RemoveMasterEdition>) -> Result {
        remove_master_edition::remove_master_edition(ctx)
    }

    pub fn update_master_edition(ctx: Context<UpdateMasterEdition>) -> Result {
        update_master_edition::update_master_edition(ctx)
    }

    pub fn approve_collection_plugin_authority(
        ctx: Context<ApproveCollectionPluginAuthority>,
    ) -> Result {
        approve_collection_plugin_authority::approve_collection_plugin_authority(ctx)
    }

    pub fn revoke_collection_plugin_authority(
        ctx: Context<RevokeCollectionPluginAuthority>,
    ) -> Result {
        revoke_collection_plugin_authority::revoke_collection_plugin_authority(ctx)
    }

    pub fn attach_add_blocker(ctx: Context<AttachAddBlocker>) -> Result {
        attach_add_blocker::attach_add_blocker(ctx)
    }

    pub fn attach_freeze_execute(ctx: Context<AttachFreezeExecute>) -> Result {
        attach_freeze_execute::attach_freeze_execute(ctx)
    }

    pub fn attach_edition(ctx: Context<AttachEdition>) -> Result {
        attach_edition::attach_edition(ctx)
    }

    pub fn attach_bubblegum_v2(ctx: Context<AttachBubblegumV2>) -> Result {
        attach_bubblegum_v2::attach_bubblegum_v2(ctx)
    }

    pub fn attach_permanent_freeze_delegate(ctx: Context<AttachPermanentFreezeDelegate>) -> Result {
        attach_permanent_freeze_delegate::attach_permanent_freeze_delegate(ctx)
    }

    pub fn attach_permanent_freeze_execute(ctx: Context<AttachPermanentFreezeExecute>) -> Result {
        attach_permanent_freeze_execute::attach_permanent_freeze_execute(ctx)
    }

    pub fn create_group(ctx: Context<CreateGroup>, name: ZcString, uri: ZcString) -> Result {
        create_group::create_group(ctx, name, uri)
    }

    pub fn close_group(ctx: Context<CloseGroup>) -> Result {
        close_group::close_group(ctx)
    }

    pub fn update_group(ctx: Context<UpdateGroup>, new_name: ZcString, new_uri: ZcString) -> Result {
        update_group::update_group(ctx, new_name, new_uri)
    }

    pub fn add_assets_to_group(ctx: Context<AddAssetsToGroup>) -> Result {
        add_assets_to_group::add_assets_to_group(ctx)
    }

    pub fn remove_assets_from_group(ctx: Context<RemoveAssetsFromGroup>) -> Result {
        remove_assets_from_group::remove_assets_from_group(ctx)
    }

    pub fn add_collections_to_group(ctx: Context<AddCollectionsToGroup>) -> Result {
        add_collections_to_group::add_collections_to_group(ctx)
    }

    pub fn remove_collections_from_group(ctx: Context<RemoveCollectionsFromGroup>) -> Result {
        remove_collections_from_group::remove_collections_from_group(ctx)
    }

    pub fn add_groups_to_group(ctx: Context<AddGroupsToGroup>) -> Result {
        add_groups_to_group::add_groups_to_group(ctx)
    }

    pub fn remove_groups_from_group(ctx: Context<RemoveGroupsFromGroup>) -> Result {
        remove_groups_from_group::remove_groups_from_group(ctx)
    }

    pub fn update_app_data(ctx: Context<UpdateAppData>) -> Result {
        update_app_data::update_app_data(ctx)
    }

    pub fn write_app_data(ctx: Context<WriteAppData>) -> Result {
        write_app_data::write_app_data(ctx)
    }

    pub fn remove_app_data(ctx: Context<RemoveAppData>) -> Result {
        remove_app_data::remove_app_data(ctx)
    }

    pub fn attach_collection_app_data(ctx: Context<AttachCollectionAppData>) -> Result {
        attach_collection_app_data::attach_collection_app_data(ctx)
    }

    pub fn update_collection_app_data(ctx: Context<UpdateCollectionAppData>) -> Result {
        update_collection_app_data::update_collection_app_data(ctx)
    }

    pub fn write_collection_app_data(ctx: Context<WriteCollectionAppData>) -> Result {
        write_collection_app_data::write_collection_app_data(ctx)
    }

    pub fn remove_collection_app_data(ctx: Context<RemoveCollectionAppData>) -> Result {
        remove_collection_app_data::remove_collection_app_data(ctx)
    }

    pub fn exercise_execute(ctx: Context<ExerciseExecute>) -> Result {
        exercise_execute::exercise_execute(ctx)
    }

    pub fn check_asset(
        ctx: Context<CheckAsset>,
        expected_owner: Address,
        expected_update_authority: Address,
        expected_name: ZcString,
        expected_uri: ZcString,
    ) -> Result {
        check_asset::check_asset(
            ctx,
            expected_owner,
            expected_update_authority,
            expected_name,
            expected_uri,
        )
    }

    pub fn check_collection(
        ctx: Context<CheckCollection>,
        expected_update_authority: Address,
        expected_name: ZcString,
        expected_uri: ZcString,
        expected_num_minted: u32,
        expected_current_size: u32,
    ) -> Result {
        check_collection::check_collection(
            ctx,
            expected_update_authority,
            expected_name,
            expected_uri,
            expected_num_minted,
            expected_current_size,
        )
    }

    pub fn check_group(
        ctx: Context<CheckGroup>,
        expected_update_authority: Address,
        expected_name: ZcString,
        expected_uri: ZcString,
    ) -> Result {
        check_group::check_group(ctx, expected_update_authority, expected_name, expected_uri)
    }

    pub fn check_freeze_delegate(ctx: Context<CheckFreezeDelegate>, expected_frozen: Bool) -> Result {
        check_freeze_delegate::check_freeze_delegate(ctx, expected_frozen)
    }

    pub fn check_master_edition(ctx: Context<CheckMasterEdition>, expected_max_supply: u32) -> Result {
        check_master_edition::check_master_edition(ctx, expected_max_supply)
    }

    pub fn check_app_data(ctx: Context<CheckAppData>) -> Result {
        check_app_data::check_app_data(ctx)
    }
}
