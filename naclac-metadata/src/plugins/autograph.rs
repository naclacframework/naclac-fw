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
    let mut data = crate::prelude::Vec::new();
    data.push(2u8); // AddPluginV1 discriminator
    data.push(plugin_type::AUTOGRAPH);
    data.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
    for (address, message) in signatures {
        data.extend_from_slice(address.as_ref());
        data.extend_from_slice(&(message.len() as u32).to_le_bytes());
        data.extend_from_slice(message.as_bytes());
    }
    data.push(0u8); // init_authority: None

    #[cfg(not(feature = "pinocchio"))]
    {
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(program, accounts, &data, signer_seeds)
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
    let asset_view = crate::asset::AssetView::from_bytes(&data)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_autograph(&data, plugin_header_offset)
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
    read_autograph(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_autograph`, shared by both backends:
/// see `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
pub fn read_autograph(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<AutographEntry>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::AUTOGRAPH)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let count_end = checked_end(offset, 4)?;
    if data.len() < count_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..count_end].try_into().unwrap());

    let mut entries = crate::prelude::Vec::with_capacity(count as usize);
    let mut cursor = count_end;
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

fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    let prefix_end = checked_end(offset, 4)?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..prefix_end].try_into().unwrap()) as usize;
    let payload_end = checked_end(prefix_end, len)?;
    if data.len() < payload_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_autograph` never panics end-to-end, including through
    /// `find_plugin_offset` and the now-`checked_end`-guarded entry point.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_autograph_never_panics() {
        let data: [u8; 40] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_autograph(&data, plugin_header_offset);
    }
}
