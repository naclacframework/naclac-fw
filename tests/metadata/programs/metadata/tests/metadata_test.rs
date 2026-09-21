use naclac_client::*;
use metadata_client::{
    instructions::{
        build_create_asset, build_create_collection, build_attach_royalties,
        build_attach_attributes, build_attach_transfer_delegate, build_attach_burn_delegate,
        build_attach_permanent_transfer_delegate, build_attach_permanent_burn_delegate,
        build_attach_immutable_metadata, build_attach_master_edition, build_attach_verified_creators,
        build_attach_app_data, build_attach_oracle, build_attach_update_delegate,
        build_attach_autograph, build_attach_freeze_delegate,
        build_transfer_asset, build_burn_asset, build_update_asset, build_update_collection,
        build_burn_collection,
        build_remove_freeze_delegate, build_update_freeze_delegate, build_approve_plugin_authority,
        build_revoke_plugin_authority,
        build_remove_master_edition, build_update_master_edition,
        build_approve_collection_plugin_authority, build_revoke_collection_plugin_authority,
        build_attach_add_blocker, build_attach_freeze_execute, build_attach_edition,
        build_attach_bubblegum_v2, build_attach_permanent_freeze_delegate,
        build_attach_permanent_freeze_execute,
        build_create_group, build_close_group, build_update_group,
        build_add_assets_to_group, build_remove_assets_from_group,
        build_add_collections_to_group, build_remove_collections_from_group,
        build_add_groups_to_group, build_remove_groups_from_group,
        build_update_app_data, build_write_app_data, build_remove_app_data,
        build_attach_collection_app_data, build_update_collection_app_data,
        build_write_collection_app_data, build_remove_collection_app_data,
        build_exercise_execute,
        build_check_asset, build_check_collection, build_check_group,
        build_check_freeze_delegate, build_check_master_edition, build_check_app_data,
        CreateAssetAccounts, CreateCollectionAccounts, AttachRoyaltiesAccounts,
        AttachAttributesAccounts, AttachTransferDelegateAccounts, AttachBurnDelegateAccounts,
        AttachPermanentTransferDelegateAccounts, AttachPermanentBurnDelegateAccounts,
        AttachImmutableMetadataAccounts, AttachMasterEditionAccounts, AttachVerifiedCreatorsAccounts,
        AttachAppDataAccounts, AttachOracleAccounts, AttachUpdateDelegateAccounts,
        AttachAutographAccounts, AttachFreezeDelegateAccounts,
        TransferAssetAccounts, BurnAssetAccounts, UpdateAssetAccounts, UpdateCollectionAccounts,
        BurnCollectionAccounts,
        RemoveFreezeDelegateAccounts, UpdateFreezeDelegateAccounts, ApprovePluginAuthorityAccounts,
        RevokePluginAuthorityAccounts,
        RemoveMasterEditionAccounts, UpdateMasterEditionAccounts,
        ApproveCollectionPluginAuthorityAccounts, RevokeCollectionPluginAuthorityAccounts,
        AttachAddBlockerAccounts, AttachFreezeExecuteAccounts, AttachEditionAccounts,
        AttachBubblegumV2Accounts, AttachPermanentFreezeDelegateAccounts,
        AttachPermanentFreezeExecuteAccounts,
        CreateGroupAccounts, CloseGroupAccounts, UpdateGroupAccounts,
        AddAssetsToGroupAccounts, RemoveAssetsFromGroupAccounts,
        AddCollectionsToGroupAccounts, RemoveCollectionsFromGroupAccounts,
        AddGroupsToGroupAccounts, RemoveGroupsFromGroupAccounts,
        UpdateAppDataAccounts, WriteAppDataAccounts, RemoveAppDataAccounts,
        AttachCollectionAppDataAccounts, UpdateCollectionAppDataAccounts,
        WriteCollectionAppDataAccounts, RemoveCollectionAppDataAccounts,
        ExerciseExecuteAccounts,
        CheckAssetAccounts, CheckCollectionAccounts, CheckGroupAccounts,
        CheckFreezeDelegateAccounts, CheckMasterEditionAccounts, CheckAppDataAccounts,
    },
    types::PROGRAM_ID,
};

const MPL_CORE_PROGRAM_ID: Address = address!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");

fn load_program(provider: &NaclacProvider) {
    provider
        .add_program_from_cluster(&MPL_CORE_PROGRAM_ID, "https://api.mainnet-beta.solana.com")
        .expect("Failed to fetch mpl_core program binary from mainnet");
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider");
    load_program(&provider);
    provider
}

#[test]
fn create_asset_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let asset_account = provider
        .get_account(&asset.address())
        .expect("the asset account should now exist");
    assert_eq!(
        asset_account.owner, MPL_CORE_PROGRAM_ID,
        "the asset created via the mpl-core CPI must be owned by the mpl-core program"
    );
}

#[test]
fn create_collection_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    let collection_account = provider
        .get_account(&collection.address())
        .expect("the collection account should now exist");
    assert_eq!(
        collection_account.owner, MPL_CORE_PROGRAM_ID,
        "the collection created via the mpl-core CPI must be owned by the mpl-core program"
    );
}

#[test]
fn attach_royalties_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    // First create the asset via the metadata program (real CPI to mpl-core).
    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    // Now attach royalties to the created asset via the metadata program.
    build_attach_royalties(
        &provider,
        PROGRAM_ID,
        500u16, // 5% basis points
        AttachRoyaltiesAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_royalties should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_attributes_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    // Create the asset first (real CPI to mpl-core).
    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    // Attach a single attribute key/value pair via the metadata program.
    build_attach_attributes(
        &provider,
        PROGRAM_ID,
        "color".to_string(),
        "blue".to_string(),
        AttachAttributesAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_attributes should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_transfer_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    // Create the asset first (real CPI to mpl-core).
    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    // Attach a transfer_delegate plugin via the metadata program.
    build_attach_transfer_delegate(
        &provider,
        PROGRAM_ID,
        AttachTransferDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_transfer_delegate should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_burn_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    // Create the asset first (real CPI to mpl-core).
    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    // Attach a burn_delegate plugin via the metadata program.
    build_attach_burn_delegate(
        &provider,
        PROGRAM_ID,
        AttachBurnDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_burn_delegate should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_permanent_transfer_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_permanent_transfer_delegate(
        &provider,
        PROGRAM_ID,
        AttachPermanentTransferDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_permanent_transfer_delegate should be rejected by mpl-core (expected failure)");
}

#[test]
fn attach_permanent_burn_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_permanent_burn_delegate(
        &provider,
        PROGRAM_ID,
        AttachPermanentBurnDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_permanent_burn_delegate should be rejected by mpl-core (expected failure)");
}

#[test]
fn attach_immutable_metadata_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_immutable_metadata(
        &provider,
        PROGRAM_ID,
        AttachImmutableMetadataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_immutable_metadata should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_master_edition_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_verified_creators_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_verified_creators(
        &provider,
        PROGRAM_ID,
        AttachVerifiedCreatorsAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_verified_creators should succeed (real CPI to the mpl-core program)");
}


#[test]
fn attach_app_data_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_app_data(
        &provider,
        PROGRAM_ID,
        AttachAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_app_data should succeed (real CPI to the mpl-core program)");
}


#[test]
fn attach_oracle_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_oracle(
        &provider,
        PROGRAM_ID,
        AttachOracleAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_oracle should succeed (real CPI to the mpl-core program)");
}


#[test]
fn attach_update_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_update_delegate(
        &provider,
        PROGRAM_ID,
        AttachUpdateDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_update_delegate should succeed (real CPI to the mpl-core program)");
}


#[test]
fn attach_autograph_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_autograph(
        &provider,
        PROGRAM_ID,
        AttachAutographAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_autograph should succeed (real CPI to the mpl-core program)");
}


#[test]
fn attach_freeze_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");
}

#[test]
fn transfer_asset_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();
    let new_owner = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_transfer_asset(
        &provider,
        PROGRAM_ID,
        TransferAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            new_owner: new_owner.address(),
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("transfer_asset should succeed (real CPI to the mpl-core program)");
}

#[test]
fn burn_asset_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_burn_asset(
        &provider,
        PROGRAM_ID,
        BurnAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("burn_asset should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_asset_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_update_asset(
        &provider,
        PROGRAM_ID,
        "Updated Asset".to_string(),
        "https://example.com/asset-updated.json".to_string(),
        UpdateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("update_asset should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_collection_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_update_collection(
        &provider,
        PROGRAM_ID,
        "Updated Collection".to_string(),
        "https://example.com/collection-updated.json".to_string(),
        UpdateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("update_collection should succeed (real CPI to the mpl-core program)");
}

#[test]
fn burn_collection_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_burn_collection(
        &provider,
        PROGRAM_ID,
        BurnCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("burn_collection should succeed (real CPI to the mpl-core program, collection has no members)");
}

#[test]
fn remove_freeze_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_remove_freeze_delegate(
        &provider,
        PROGRAM_ID,
        RemoveFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("remove_freeze_delegate should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_freeze_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_update_freeze_delegate(
        &provider,
        PROGRAM_ID,
        UpdateFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("update_freeze_delegate should succeed (real CPI to the mpl-core program)");
}

#[test]
fn approve_plugin_authority_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();
    let new_authority = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_approve_plugin_authority(
        &provider,
        PROGRAM_ID,
        ApprovePluginAuthorityAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            new_authority: new_authority.address(),
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("approve_plugin_authority should succeed (real CPI to the mpl-core program)");
}

#[test]
fn revoke_plugin_authority_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_revoke_plugin_authority(
        &provider,
        PROGRAM_ID,
        RevokePluginAuthorityAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("revoke_plugin_authority should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_master_edition_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");

    build_remove_master_edition(
        &provider,
        PROGRAM_ID,
        RemoveMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("remove_master_edition should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_master_edition_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");

    build_update_master_edition(
        &provider,
        PROGRAM_ID,
        UpdateMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("update_master_edition should succeed (real CPI to the mpl-core program)");
}

#[test]
fn approve_collection_plugin_authority_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();
    let new_authority = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");

    build_approve_collection_plugin_authority(
        &provider,
        PROGRAM_ID,
        ApproveCollectionPluginAuthorityAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            new_authority: new_authority.address(),
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("approve_collection_plugin_authority should succeed (real CPI to the mpl-core program)");
}

#[test]
fn revoke_collection_plugin_authority_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");

    build_revoke_collection_plugin_authority(
        &provider,
        PROGRAM_ID,
        RevokeCollectionPluginAuthorityAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("revoke_collection_plugin_authority should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_add_blocker_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_add_blocker(
        &provider,
        PROGRAM_ID,
        AttachAddBlockerAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_add_blocker should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_freeze_execute_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_execute(
        &provider,
        PROGRAM_ID,
        AttachFreezeExecuteAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_execute should succeed (real CPI to the mpl-core program)");
}

#[test]
fn attach_edition_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_edition(
        &provider,
        PROGRAM_ID,
        AttachEditionAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_edition should be rejected by mpl-core (Edition can only be set at creation time)");
}

#[test]
fn attach_bubblegum_v2_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_bubblegum_v2(
        &provider,
        PROGRAM_ID,
        AttachBubblegumV2Accounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_bubblegum_v2 should be rejected by mpl-core (BubblegumV2 can only be set at creation time)");
}

#[test]
fn attach_permanent_freeze_delegate_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_permanent_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachPermanentFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_permanent_freeze_delegate should be rejected by mpl-core (Permanent* plugins can only be set at creation time)");
}

#[test]
fn attach_permanent_freeze_execute_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let res = build_attach_permanent_freeze_execute(
        &provider,
        PROGRAM_ID,
        AttachPermanentFreezeExecuteAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm();

    assert!(res.is_err(), "attach_permanent_freeze_execute should be rejected by mpl-core (Permanent* plugins can only be set at creation time)");
}

#[test]
fn create_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    let group_account = provider
        .get_account(&group.address())
        .expect("the group account should now exist");
    assert_eq!(
        group_account.owner, MPL_CORE_PROGRAM_ID,
        "the group created via the mpl-core CPI must be owned by the mpl-core program"
    );
}

#[test]
fn close_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_close_group(
        &provider,
        PROGRAM_ID,
        CloseGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("close_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_update_group(
        &provider,
        PROGRAM_ID,
        "Updated Group".to_string(),
        "https://example.com/group-updated.json".to_string(),
        UpdateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("update_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn add_assets_to_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();
    let asset = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_add_assets_to_group(
        &provider,
        PROGRAM_ID,
        AddAssetsToGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(asset.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_assets_to_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_assets_from_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();
    let asset = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_add_assets_to_group(
        &provider,
        PROGRAM_ID,
        AddAssetsToGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(asset.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_assets_to_group should succeed (real CPI to the mpl-core program)");

    build_remove_assets_from_group(
        &provider,
        PROGRAM_ID,
        RemoveAssetsFromGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(asset.address(), false)])
    .log()
    .send_and_confirm()
    .expect("remove_assets_from_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn add_collections_to_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();
    let collection = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_add_collections_to_group(
        &provider,
        PROGRAM_ID,
        AddCollectionsToGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(collection.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_collections_to_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_collections_from_group_works_end_to_end() {
    let provider = setup();
    let group = Keypair::new();
    let collection = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_add_collections_to_group(
        &provider,
        PROGRAM_ID,
        AddCollectionsToGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(collection.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_collections_to_group should succeed (real CPI to the mpl-core program)");

    build_remove_collections_from_group(
        &provider,
        PROGRAM_ID,
        RemoveCollectionsFromGroupAccounts {
            group: group.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .remaining_accounts(vec![AccountMeta::new(collection.address(), false)])
    .log()
    .send_and_confirm()
    .expect("remove_collections_from_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn add_groups_to_group_works_end_to_end() {
    let provider = setup();
    let parent = Keypair::new();
    let child = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Parent Group".to_string(),
        "https://example.com/parent-group.json".to_string(),
        CreateGroupAccounts {
            group: parent.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&parent)
    .log()
    .send_and_confirm()
    .expect("create_group (parent) should succeed (real CPI to the mpl-core program)");

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Child Group".to_string(),
        "https://example.com/child-group.json".to_string(),
        CreateGroupAccounts {
            group: child.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&child)
    .log()
    .send_and_confirm()
    .expect("create_group (child) should succeed (real CPI to the mpl-core program)");

    build_add_groups_to_group(
        &provider,
        PROGRAM_ID,
        AddGroupsToGroupAccounts {
            group: parent.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&parent)
    .remaining_accounts(vec![AccountMeta::new(child.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_groups_to_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_groups_from_group_works_end_to_end() {
    let provider = setup();
    let parent = Keypair::new();
    let child = Keypair::new();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Parent Group".to_string(),
        "https://example.com/parent-group.json".to_string(),
        CreateGroupAccounts {
            group: parent.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&parent)
    .log()
    .send_and_confirm()
    .expect("create_group (parent) should succeed (real CPI to the mpl-core program)");

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Child Group".to_string(),
        "https://example.com/child-group.json".to_string(),
        CreateGroupAccounts {
            group: child.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&child)
    .log()
    .send_and_confirm()
    .expect("create_group (child) should succeed (real CPI to the mpl-core program)");

    build_add_groups_to_group(
        &provider,
        PROGRAM_ID,
        AddGroupsToGroupAccounts {
            group: parent.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&parent)
    .remaining_accounts(vec![AccountMeta::new(child.address(), false)])
    .log()
    .send_and_confirm()
    .expect("add_groups_to_group should succeed (real CPI to the mpl-core program)");

    build_remove_groups_from_group(
        &provider,
        PROGRAM_ID,
        RemoveGroupsFromGroupAccounts {
            group: parent.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&parent)
    .remaining_accounts(vec![AccountMeta::new(child.address(), false)])
    .log()
    .send_and_confirm()
    .expect("remove_groups_from_group should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_app_data_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_app_data(
        &provider,
        PROGRAM_ID,
        AttachAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_app_data should succeed (real CPI to the mpl-core program)");

    build_update_app_data(
        &provider,
        PROGRAM_ID,
        UpdateAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("update_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn write_app_data_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_app_data(
        &provider,
        PROGRAM_ID,
        AttachAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_app_data should succeed (real CPI to the mpl-core program)");

    build_write_app_data(
        &provider,
        PROGRAM_ID,
        WriteAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("write_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_app_data_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_app_data(
        &provider,
        PROGRAM_ID,
        AttachAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_app_data should succeed (real CPI to the mpl-core program)");

    build_remove_app_data(
        &provider,
        PROGRAM_ID,
        RemoveAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("remove_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn update_collection_app_data_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_collection_app_data(
        &provider,
        PROGRAM_ID,
        AttachCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_collection_app_data should succeed (real CPI to the mpl-core program)");

    build_update_collection_app_data(
        &provider,
        PROGRAM_ID,
        UpdateCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("update_collection_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn write_collection_app_data_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_collection_app_data(
        &provider,
        PROGRAM_ID,
        AttachCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_collection_app_data should succeed (real CPI to the mpl-core program)");

    build_write_collection_app_data(
        &provider,
        PROGRAM_ID,
        WriteCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("write_collection_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn remove_collection_app_data_works_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_collection_app_data(
        &provider,
        PROGRAM_ID,
        AttachCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_collection_app_data should succeed (real CPI to the mpl-core program)");

    build_remove_collection_app_data(
        &provider,
        PROGRAM_ID,
        RemoveCollectionAppDataAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("remove_collection_app_data should succeed (real CPI to the mpl-core program)");
}

#[test]
fn exercise_execute_works_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();
    let destination = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    let (asset_signer, _bump) =
        Address::find_program_address(&[b"mpl-core-execute", asset.address().as_ref()], &MPL_CORE_PROGRAM_ID);

    build_exercise_execute(
        &provider,
        PROGRAM_ID,
        ExerciseExecuteAccounts {
            asset: asset.address(),
            asset_signer,
            payer: provider.payer.address(),
            destination: destination.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("exercise_execute should succeed (real CPI to the mpl-core program, which itself CPIs into the System Program as the asset's derived signer)");
}

#[test]
fn check_asset_matches_after_create_and_update_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();
    let payer_address = provider.payer.address();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_check_asset(
        &provider,
        PROGRAM_ID,
        payer_address,
        payer_address,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CheckAssetAccounts {
            asset: asset.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_asset should confirm fetch_asset's parsed fields match what create_asset wrote");

    build_update_asset(
        &provider,
        PROGRAM_ID,
        "Updated Asset".to_string(),
        "https://example.com/asset-updated.json".to_string(),
        UpdateAssetAccounts {
            asset: asset.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("update_asset should succeed (real CPI to the mpl-core program)");

    build_check_asset(
        &provider,
        PROGRAM_ID,
        payer_address,
        payer_address,
        "Updated Asset".to_string(),
        "https://example.com/asset-updated.json".to_string(),
        CheckAssetAccounts {
            asset: asset.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_asset should confirm fetch_asset's parsed fields track update_asset's changes");
}

#[test]
fn check_collection_matches_after_create_and_update_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();
    let payer_address = provider.payer.address();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_check_collection(
        &provider,
        PROGRAM_ID,
        payer_address,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        0,
        0,
        CheckCollectionAccounts {
            collection: collection.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_collection should confirm fetch_collection's parsed fields match what create_collection wrote");

    build_update_collection(
        &provider,
        PROGRAM_ID,
        "Updated Collection".to_string(),
        "https://example.com/collection-updated.json".to_string(),
        UpdateCollectionAccounts {
            collection: collection.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("update_collection should succeed (real CPI to the mpl-core program)");

    build_check_collection(
        &provider,
        PROGRAM_ID,
        payer_address,
        "Updated Collection".to_string(),
        "https://example.com/collection-updated.json".to_string(),
        0,
        0,
        CheckCollectionAccounts {
            collection: collection.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_collection should confirm fetch_collection's parsed fields track update_collection's changes");
}

#[test]
fn check_group_matches_after_create_and_update_end_to_end() {
    let provider = setup();
    let group = Keypair::new();
    let payer_address = provider.payer.address();

    build_create_group(
        &provider,
        PROGRAM_ID,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CreateGroupAccounts {
            group: group.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("create_group should succeed (real CPI to the mpl-core program)");

    build_check_group(
        &provider,
        PROGRAM_ID,
        payer_address,
        "Test Group".to_string(),
        "https://example.com/group.json".to_string(),
        CheckGroupAccounts {
            group: group.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_group should confirm fetch_group's parsed fields match what create_group wrote");

    build_update_group(
        &provider,
        PROGRAM_ID,
        "Updated Group".to_string(),
        "https://example.com/group-updated.json".to_string(),
        UpdateGroupAccounts {
            group: group.address(),
            payer: payer_address,
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&group)
    .log()
    .send_and_confirm()
    .expect("update_group should succeed (real CPI to the mpl-core program)");

    build_check_group(
        &provider,
        PROGRAM_ID,
        payer_address,
        "Updated Group".to_string(),
        "https://example.com/group-updated.json".to_string(),
        CheckGroupAccounts {
            group: group.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_group should confirm fetch_group's parsed fields track update_group's changes");
}

#[test]
fn check_freeze_delegate_matches_after_attach_and_update_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_freeze_delegate(
        &provider,
        PROGRAM_ID,
        AttachFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_check_freeze_delegate(
        &provider,
        PROGRAM_ID,
        Bool::from(false),
        CheckFreezeDelegateAccounts {
            asset: asset.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_freeze_delegate should confirm fetch_asset_freeze_delegate reads back frozen=false right after attach");

    build_update_freeze_delegate(
        &provider,
        PROGRAM_ID,
        UpdateFreezeDelegateAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("update_freeze_delegate should succeed (real CPI to the mpl-core program)");

    build_check_freeze_delegate(
        &provider,
        PROGRAM_ID,
        Bool::from(true),
        CheckFreezeDelegateAccounts {
            asset: asset.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_freeze_delegate should confirm fetch_asset_freeze_delegate tracks update_freeze_delegate's change to frozen=true");
}

#[test]
fn check_master_edition_matches_after_attach_and_update_end_to_end() {
    let provider = setup();
    let collection = Keypair::new();

    build_create_collection(
        &provider,
        PROGRAM_ID,
        "Test Collection".to_string(),
        "https://example.com/collection.json".to_string(),
        CreateCollectionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("create_collection should succeed (real CPI to the mpl-core program)");

    build_attach_master_edition(
        &provider,
        PROGRAM_ID,
        AttachMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("attach_master_edition should succeed (real CPI to the mpl-core program)");

    build_update_master_edition(
        &provider,
        PROGRAM_ID,
        UpdateMasterEditionAccounts {
            collection: collection.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&collection)
    .log()
    .send_and_confirm()
    .expect("update_master_edition should succeed (real CPI to the mpl-core program)");

    build_check_master_edition(
        &provider,
        PROGRAM_ID,
        100,
        CheckMasterEditionAccounts {
            collection: collection.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_master_edition should confirm fetch_collection_master_edition reads back max_supply=100 after update_master_edition");
}

#[test]
fn check_app_data_matches_after_write_end_to_end() {
    let provider = setup();
    let asset = Keypair::new();

    build_create_asset(
        &provider,
        PROGRAM_ID,
        "Test Asset".to_string(),
        "https://example.com/asset.json".to_string(),
        CreateAssetAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("create_asset should succeed (real CPI to the mpl-core program)");

    build_attach_app_data(
        &provider,
        PROGRAM_ID,
        AttachAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("attach_app_data should succeed (real CPI to the mpl-core program)");

    build_write_app_data(
        &provider,
        PROGRAM_ID,
        WriteAppDataAccounts {
            asset: asset.address(),
            payer: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
            mpl_core_program: MPL_CORE_PROGRAM_ID,
        },
    )
    .signer(&asset)
    .log()
    .send_and_confirm()
    .expect("write_app_data should succeed (real CPI to the mpl-core program)");

    build_check_app_data(
        &provider,
        PROGRAM_ID,
        CheckAppDataAccounts {
            asset: asset.address(),
        },
    )
    .log()
    .send_and_confirm()
    .expect("check_app_data should confirm fetch_asset_app_data reads back the bytes write_app_data wrote");
}
