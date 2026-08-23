// ===========================================================================
// plugins/autograph.rs — the `Autograph` plugin
// ===========================================================================

//! `Autograph` — attaches to an `Asset`, holding a list of address/message
//! "signatures" left by holders. Real layout verified against `mpl-core`'s
//! `generated::types::{Autograph, AutographSignature}`:
//! `Autograph { signatures: Vec<AutographSignature> }`,
//! `AutographSignature { address: Pubkey, message: String }` — genuinely
//! variable-width per element (unlike `VerifiedCreators`' fixed-width
//! entries), so reading requires a linear scan, same shape as
//! `attributes.rs`'s walk.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// One entry of `Autograph.signatures` — backend-neutral, owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutographEntry {
    pub address: Address,
    pub message: crate::prelude::String,
}

/// Attaches `Autograph` to an `Asset` via `add_plugin`.
pub fn attach_autograph_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signatures: &[(Address, &str)],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::Autograph(::mpl_core::types::Autograph {
            signatures: signatures
                .iter()
                .map(|(address, message)| ::mpl_core::types::AutographSignature {
                    address: *address,
                    message: message.to_string(),
                })
                .collect(),
        });
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut payload = crate::prelude::Vec::new();
        payload.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for (address, message) in signatures {
            payload.extend_from_slice(address.as_ref());
            payload.extend_from_slice(&(message.len() as u32).to_le_bytes());
            payload.extend_from_slice(message.as_bytes());
        }
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::AUTOGRAPH,
            &payload,
            signer_seeds,
        )
    }
}

/// Reads an `Asset`'s `Autograph` plugin, if attached. Callable identically
/// on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_autograph(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AutographEntry>>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.autograph.map(|p| {
        p.autograph
            .signatures
            .into_iter()
            .map(|s| AutographEntry {
                address: s.address,
                message: s.message,
            })
            .collect()
    }))
}

/// Reads an `Asset`'s `Autograph` plugin, if attached. Callable identically
/// on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_autograph(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AutographEntry>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::AUTOGRAPH)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());

    let mut entries = crate::prelude::Vec::with_capacity(count as usize);
    let mut cursor = offset + 4;
    for _ in 0..count {
        if data.len() < cursor + 32 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let bytes: [u8; 32] = data[cursor..cursor + 32].try_into().unwrap();
        let address = Address::new_from_array(bytes);
        cursor += 32;

        let message_len = read_borsh_string_len(data, cursor)?;
        let message_start = cursor + 4;
        // SAFETY: bounds validated by `read_borsh_string_len`; Core's own
        // write path always encodes `message` as a valid Borsh (UTF-8)
        // `String`.
        let message = unsafe {
            core::str::from_utf8_unchecked(&data[message_start..message_start + message_len])
        };
        cursor = message_start + message_len;

        entries.push(AutographEntry {
            address,
            message: message.into(),
        });
    }

    Ok(Some(entries))
}

#[cfg(feature = "pinocchio")]
fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    if data.len() < offset + 4 + len {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}
