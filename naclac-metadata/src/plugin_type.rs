// ===========================================================================
// plugin_type.rs — raw `PluginType` discriminants, both backends
// ===========================================================================

//! Raw `PluginType` discriminants (Borsh unit-enum order, verified against
//! `mpl-core`'s real `generated::types::PluginType`). Available on both
//! backends: `solana` code can use the real `::mpl_core::types::PluginType`
//! enum directly for most purposes, but a few functions (e.g.
//! `plugin_authority.rs`'s `approve_asset_plugin_authority_signed`) need a
//! backend-uniform argument type so their call signature matches on both
//! backends — these raw constants are that shared representation.

pub const ROYALTIES: u8 = 0;
pub const FREEZE_DELEGATE: u8 = 1;
pub const BURN_DELEGATE: u8 = 2;
pub const TRANSFER_DELEGATE: u8 = 3;
pub const UPDATE_DELEGATE: u8 = 4;
pub const PERMANENT_FREEZE_DELEGATE: u8 = 5;
pub const ATTRIBUTES: u8 = 6;
pub const PERMANENT_TRANSFER_DELEGATE: u8 = 7;
pub const PERMANENT_BURN_DELEGATE: u8 = 8;
pub const EDITION: u8 = 9;
pub const MASTER_EDITION: u8 = 10;
pub const ADD_BLOCKER: u8 = 11;
pub const IMMUTABLE_METADATA: u8 = 12;
pub const VERIFIED_CREATORS: u8 = 13;
pub const AUTOGRAPH: u8 = 14;
pub const BUBBLEGUM_V2: u8 = 15;
pub const FREEZE_EXECUTE: u8 = 16;
pub const PERMANENT_FREEZE_EXECUTE: u8 = 17;
pub const GROUPS: u8 = 18;

/// Converts a raw discriminant back into the real `::mpl_core::types::PluginType`
/// enum, `solana`-only. Panics on an out-of-range value — every call site
/// passes one of this module's own constants, so an out-of-range value
/// means a real bug in the caller, not a case worth a `Result`.
#[cfg(not(feature = "pinocchio"))]
pub fn to_real_plugin_type(plugin_type: u8) -> ::mpl_core::types::PluginType {
    match plugin_type {
        ROYALTIES => ::mpl_core::types::PluginType::Royalties,
        FREEZE_DELEGATE => ::mpl_core::types::PluginType::FreezeDelegate,
        BURN_DELEGATE => ::mpl_core::types::PluginType::BurnDelegate,
        TRANSFER_DELEGATE => ::mpl_core::types::PluginType::TransferDelegate,
        UPDATE_DELEGATE => ::mpl_core::types::PluginType::UpdateDelegate,
        PERMANENT_FREEZE_DELEGATE => ::mpl_core::types::PluginType::PermanentFreezeDelegate,
        ATTRIBUTES => ::mpl_core::types::PluginType::Attributes,
        PERMANENT_TRANSFER_DELEGATE => ::mpl_core::types::PluginType::PermanentTransferDelegate,
        PERMANENT_BURN_DELEGATE => ::mpl_core::types::PluginType::PermanentBurnDelegate,
        EDITION => ::mpl_core::types::PluginType::Edition,
        MASTER_EDITION => ::mpl_core::types::PluginType::MasterEdition,
        ADD_BLOCKER => ::mpl_core::types::PluginType::AddBlocker,
        IMMUTABLE_METADATA => ::mpl_core::types::PluginType::ImmutableMetadata,
        VERIFIED_CREATORS => ::mpl_core::types::PluginType::VerifiedCreators,
        AUTOGRAPH => ::mpl_core::types::PluginType::Autograph,
        BUBBLEGUM_V2 => ::mpl_core::types::PluginType::BubblegumV2,
        FREEZE_EXECUTE => ::mpl_core::types::PluginType::FreezeExecute,
        PERMANENT_FREEZE_EXECUTE => ::mpl_core::types::PluginType::PermanentFreezeExecute,
        GROUPS => ::mpl_core::types::PluginType::Groups,
        _ => panic!("plugin_type: out-of-range PluginType discriminant {plugin_type}"),
    }
}
