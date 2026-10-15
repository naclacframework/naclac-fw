//! One file per Metaplex Core plugin type — see `plugin.rs` for the shared
//! `add_plugin`/`add_collection_plugin` CPI mechanics every file here
//! builds on, and `plugin_registry.rs` for how a plugin's raw bytes are
//! located on the `pinocchio` backend.

pub mod add_blocker;
pub mod attributes;
pub mod autograph;
pub mod bubblegum_v2;
pub mod burn_delegate;
pub mod edition;
pub mod freeze_delegate;
pub mod freeze_execute;
pub mod groups;
pub mod immutable_metadata;
pub mod master_edition;
pub mod permanent_burn_delegate;
pub mod permanent_freeze_delegate;
pub mod permanent_freeze_execute;
pub mod permanent_transfer_delegate;
pub mod royalties;
pub mod transfer_delegate;
pub mod update_delegate;
pub mod verified_creators;

pub use {
    add_blocker::*, attributes::*, autograph::*, bubblegum_v2::*, burn_delegate::*, edition::*,
    freeze_delegate::*, freeze_execute::*, groups::*, immutable_metadata::*, master_edition::*,
    permanent_burn_delegate::*, permanent_freeze_delegate::*, permanent_freeze_execute::*,
    permanent_transfer_delegate::*, royalties::*, transfer_delegate::*, update_delegate::*,
    verified_creators::*,
};
