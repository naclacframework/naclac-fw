use {
    super::{
        map_type_to_rust_cpi, map_type_to_rust_with_prefix, references_defined_type, render_docs,
        sdk_core_alias,
    },
    crate::Idl,
    heck::{AsSnakeCase, ToUpperCamelCase},
    std::{fs, path::Path},
};

const BORSH_IMPORTS: &str = "#[cfg(all(feature = \"borsh\", feature = \"offchain\"))]\n\
     use crate::sdk_core_offchain::borsh::{BorshDeserialize, BorshSerialize};\n\
     #[cfg(all(feature = \"borsh\", not(feature = \"offchain\")))]\n\
     use crate::sdk_core_cpi::borsh::{BorshDeserialize, BorshSerialize};\n";

const TYPEDEFS_IMPORT: &str = "use crate::types::typedefs::*;\n";

/// True when the generated client must hold every account in one `src/state.rs`
/// instead of a `src/state/` folder with a file per account. The per-account
/// file is named after the account, so an account whose snake_case name is
/// `state` would otherwise become `state/state.rs`, a module inside a module of
/// the same name, which clippy rejects under `-D warnings`. `generate_rust_sdk`
/// uses this to decide whether to create `src/state/`, so it must stay the
/// single source of truth for that decision.
pub(crate) fn collapses_into_one_file(idl: &Idl) -> bool {
    idl.accounts
        .iter()
        .any(|acc| AsSnakeCase(&acc.name).to_string() == "state")
}

pub fn generate_state(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.accounts.is_empty() {
        return Ok(());
    }

    let collapsed = collapses_into_one_file(idl);
    let mut collapsed_items = String::new();

    if !collapsed {
        let mut state_mod = header.to_string();
        for acc in &idl.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            state_mod.push_str(&format!("pub mod {};\n", acc_snake));
            state_mod.push_str(&format!("pub use {}::*;\n", acc_snake));
        }
        fs::write(clients_dir.join("src/state/mod.rs"), state_mod).unwrap();
    }

    for acc in &idl.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        let acc_camel = acc.name.to_upper_camel_case();

        let uses_typedefs = acc.ty.fields.iter().any(|f| references_defined_type(&f.ty));
        let mut comp_content = String::new();

        // Field types (e.g. `Address`/`Bool`) genuinely differ between
        // `sdk_core_offchain` and `sdk_core_cpi` — not interchangeable, just
        // same-shaped — so the whole struct must be duplicated per branch
        // rather than sharing one body. Gated on `feature = "cpi"` / `feature
        // = "offchain"` independently (not a mutually-exclusive `offchain`/
        // `not(offchain)` pair) so both can coexist under their own distinct
        // name when a crate is reached both ways at once (e.g. a normal
        // `cpi`-only dependency and an `offchain`-only dev-dependency on the
        // same package, unifying both features) — same reasoning as
        // `types/typedefs.rs`'s struct branch, which this mirrors.
        for for_cpi in [false, true] {
            let sdk_core = sdk_core_alias(for_cpi);
            let cfg = if for_cpi {
                "#[cfg(feature = \"cpi\")]\n"
            } else {
                "#[cfg(feature = \"offchain\")]\n"
            };
            // Distinct names, not just distinct cfg gates — see
            // `map_type_to_rust_inner`'s comment on the matching `Cpi`-suffix
            // it appends for every reference to this type.
            let struct_name = if for_cpi {
                format!("{}Cpi", acc_camel)
            } else {
                acc_camel.clone()
            };

            // Field types (mapped once — both the Borsh and zero-copy
            // declarations below need them, and so does the `syn`-based
            // padded-field reconstruction).
            let mapped_field_tys: Vec<String> = acc
                .ty
                .fields
                .iter()
                .map(|field| {
                    if for_cpi {
                        map_type_to_rust_cpi(&field.ty, idl.is_zero_copy, "")
                    } else {
                        map_type_to_rust_with_prefix(&field.ty, idl.is_zero_copy, "")
                    }
                })
                .collect();

            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(any(feature = \"borsh\", feature = \"wincode\"))]\n");
            comp_content.push_str(&render_docs(&acc.docs, ""));
            comp_content.push_str(&format!(
                "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                 #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                 #[cfg_attr(feature = \"wincode\", derive(Clone, Debug, {sdk_core}::wincode::SchemaWrite, {sdk_core}::wincode::SchemaRead))]\n\
                 #[cfg_attr(feature = \"wincode\", wincode(crate = \"{sdk_core}::wincode\"))]\n\
                 pub struct {name} {{\n",
                sdk_core = sdk_core,
                name = struct_name
            ));
            for (field, field_ty) in acc.ty.fields.iter().zip(&mapped_field_tys) {
                let field_snake = AsSnakeCase(&field.name).to_string();
                comp_content.push_str(&render_docs(&field.docs, "    "));
                comp_content.push_str(&format!("    pub {}: {},\n", field_snake, field_ty));
            }
            comp_content.push_str("}\n\n");

            let zero_copy_field_tys: Vec<String> = acc
                .ty
                .fields
                .iter()
                .map(|field| {
                    crate::rust::map_zero_copy_field_type(
                        &field.ty,
                        "",
                        for_cpi,
                        &idl.defined_types,
                    )
                })
                .collect();
            let mut scratch_fields_src = String::new();
            for (field, field_ty) in acc.ty.fields.iter().zip(&zero_copy_field_tys) {
                let field_snake = AsSnakeCase(&field.name).to_string();
                scratch_fields_src.push_str(&format!("pub {}: {},\n", field_snake, field_ty));
            }
            let scratch_src = format!("struct {} {{ {} }}", struct_name, scratch_fields_src);
            let parsed_struct: syn::ItemStruct = syn::parse_str(&scratch_src).unwrap_or_else(|e| {
                panic!(
                    "state: generated account struct `{}` failed to parse as a \
                     syn::ItemStruct (bug in naclac-client-gen's type-string generation, \
                     not in the source IDL): {e}",
                    struct_name
                )
            });
            let struct_ident: syn::Ident = syn::parse_str(&struct_name)
                .expect("state: account struct name must be a valid identifier");
            let core: syn::Path =
                syn::parse_str(sdk_core).expect("state: sdk_core alias must be a valid path");
            let layout_checks = naclac_syn::zc_codegen::struct_checks::generate(
                &struct_ident,
                &parsed_struct.fields,
                &crate::rust::zero_copy_gate(for_cpi),
                &core,
            );
            let struct_fields = &parsed_struct.fields;
            let struct_item_tokens = quote::quote! {
                #[derive(Copy, Clone, Debug)]
                #[repr(C, packed)]
                pub struct #struct_ident #struct_fields
            };
            let struct_file: syn::File = syn::parse2(struct_item_tokens)
                .expect("state: generated account struct item must parse as a syn::File");

            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
            comp_content.push_str(&render_docs(&acc.docs, ""));
            comp_content.push_str(&prettyplease::unparse(&struct_file));
            comp_content.push('\n');

            let layout_file: syn::File = syn::parse2(layout_checks)
                .expect("state: struct_checks::generate output must parse as a syn::File");
            comp_content.push_str(&prettyplease::unparse(&layout_file));
            comp_content.push('\n');

            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
            comp_content.push_str(&format!(
                "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
                sdk_core, struct_name
            ));
            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
            comp_content.push_str(&format!(
                "unsafe impl {}::bytemuck::Pod for {} {{}}\n",
                sdk_core, struct_name
            ));
            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
            comp_content.push_str(&format!(
                "impl core::default::Default for {1} {{\n    fn default() -> Self {{\n        {0}::bytemuck::Zeroable::zeroed()\n    }}\n}}\n\n",
                sdk_core, struct_name
            ));
        }

        // No alias from the plain name to `{name}Cpi`: `offchain` and `cpi`
        // aren't mutually exclusive at the Cargo level (feature unification
        // can activate both in one build), and an alias gated on
        // `not(feature = "offchain")` would silently vanish the moment
        // offchain is also active, leaving the plain name pointing at the
        // *offchain* struct instead -- exactly the ambiguity this file's
        // independent per-branch naming exists to avoid (see the loop
        // above). Every on-chain CPI caller must reference the
        // `Cpi`-suffixed name explicitly, the same way `instructions/mod.rs`
        // requires `{Ix}CpiIxArgs` rather than a bare `{Ix}IxArgs` alias.

        // Discriminator check helper
        if let Some(disc) = &acc.discriminator {
            let disc_bytes = disc
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let disc_name = acc.name.to_uppercase().replace(' ', "_");
            comp_content.push_str(&format!(
                "\n/// On-chain discriminator for `{}` accounts.\n\
                 pub const {}_DISCRIMINATOR: &[u8] = &[{}];\n",
                acc_camel, disc_name, disc_bytes,
            ));

            // A single `AccountFetcher::fetch::<{acc_camel}>()` call site works
            // correctly regardless of Borsh vs zero-copy mode: the decode
            // strategy is picked here, at THIS type's own compile time (via the
            // generated client crate's own `borsh` feature, set correctly from
            // the real IDL at generation time), not guessed at runtime.
            comp_content.push_str(&format!(
                r#"#[cfg(feature = "offchain")]
#[cfg(not(any(feature = "borsh", feature = "wincode")))]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<&'static [u8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_pod_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}

#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<&'static [u8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_borsh_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}

#[cfg(feature = "offchain")]
#[cfg(feature = "wincode")]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<&'static [u8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_wincode_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}
"#,
                acc_camel = acc_camel,
                disc_name = disc_name,
            ));
        }

        // Network-fetching convenience functions. Identical in both modes —
        // each delegates entirely to `{acc_camel}`'s own `NaclacDecode` impl
        // above (the single source of truth for how to decode this type), so
        // there's no independent borsh/bytemuck logic here to drift out of
        // sync with it. Kept as named, discoverable per-type functions
        // (rather than only the generic `AccountFetcher`) because that's the
        // actual value of a generated SDK: auto-specializing the generic
        // framework tooling for this one program's own types. Anyone with
        // account bytes from somewhere other than `provider` (a simulation
        // result, a cached blob, ...) can decode them directly via
        // `<{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&bytes)`.
        comp_content.push_str(&format!(
            r#"#[cfg(feature = "offchain")]
/// Fetches and decodes a `{acc_camel}` account from the chain.
pub fn fetch_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<{acc_camel}, naclac_client::NaclacClientError> {{
    let raw = provider.get_account_data(address)?;
    <{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&raw)
}}

#[cfg(feature = "offchain")]
/// Fetches a `{acc_camel}` account. Returns `None` if the account does not exist.
pub fn fetch_maybe_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<Option<{acc_camel}>, naclac_client::NaclacClientError> {{
    match fetch_{acc_snake}(provider, address) {{
        Ok(a) => Ok(Some(a)),
        Err(naclac_client::NaclacClientError::AccountNotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }}
}}

#[cfg(feature = "offchain")]
/// Fetches and decodes every on-chain `{acc_camel}` account owned by `program_id`.
pub fn fetch_all_{acc_snake}(
    provider: &naclac_client::NaclacProvider,
    program_id: &naclac_client::Address,
) -> Result<crate::sdk_core_offchain::Vec<(naclac_client::Address, {acc_camel})>, naclac_client::NaclacClientError> {{
    let raw_accounts = provider.get_program_accounts(program_id, Some({disc_name}_DISCRIMINATOR))?;
    let mut results = crate::sdk_core_offchain::Vec::new();
    for (address, data) in raw_accounts {{
        if let Ok(value) = <{acc_camel} as naclac_client::NaclacDecode>::naclac_decode(&data) {{
            results.push((address, value));
        }}
    }}
    Ok(results)
}}
"#,
            acc_camel = acc_camel,
            acc_snake = acc_snake,
            disc_name = acc.name.to_uppercase().replace(' ', "_"),
        ));

        if collapsed {
            if !collapsed_items.is_empty() {
                collapsed_items.push('\n');
            }
            collapsed_items.push_str(&comp_content);
        } else {
            let typedefs = if uses_typedefs { TYPEDEFS_IMPORT } else { "" };
            fs::write(
                clients_dir.join(format!("src/state/{}.rs", acc_snake)),
                format!("{header}{BORSH_IMPORTS}{typedefs}\n{comp_content}"),
            )
            .unwrap();
        }
    }

    if collapsed {
        let any_typedefs = idl
            .accounts
            .iter()
            .any(|acc| acc.ty.fields.iter().any(|f| references_defined_type(&f.ty)));
        let typedefs = if any_typedefs { TYPEDEFS_IMPORT } else { "" };
        fs::write(
            clients_dir.join("src/state.rs"),
            format!("{header}{BORSH_IMPORTS}{typedefs}\n{collapsed_items}"),
        )
        .unwrap();
        eprintln!(
            "ℹ️  naclac: this program has an account named `State`, so its generated Rust client keeps every account in one `src/state.rs` \
             instead of a `src/state/` folder (a `state/state.rs` module would trip clippy's `module_inception`). Nothing to do; \
             rename the account if you want the usual one-file-per-account layout."
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::{collapses_into_one_file, generate_state},
        crate::Idl,
        serde_json::json,
        std::{fs, path::PathBuf},
    };

    fn idl_with_accounts(names: &[&str]) -> Idl {
        let accounts: Vec<_> = names
            .iter()
            .map(|name| {
                json!({
                    "name": name,
                    "type": { "kind": "struct", "fields": [{ "name": "value", "type": "u64" }] },
                })
            })
            .collect();
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "p", "version": "0.1.0", "description": "d", "serialization": "borsh" },
            "instructions": [],
            "accounts": accounts,
            "events": [],
            "errors": [],
            "constants": [],
        });
        Idl::from_json(&idl.to_string()).unwrap()
    }

    fn scratch_client(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("naclac_state_{test}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    #[test]
    fn only_an_account_whose_snake_case_name_is_state_collapses_the_layout() {
        assert!(collapses_into_one_file(&idl_with_accounts(&["State"])));
        assert!(collapses_into_one_file(&idl_with_accounts(&[
            "Vault", "State"
        ])));
        assert!(!collapses_into_one_file(&idl_with_accounts(&[
            "Vault",
            "GlobalState"
        ])));
        assert!(!collapses_into_one_file(&idl_with_accounts(&["Stateful"])));
    }

    #[test]
    fn a_state_account_puts_every_account_in_one_state_rs() {
        let dir = scratch_client("collapsed");
        let idl = idl_with_accounts(&["State", "Vault"]);
        generate_state(&idl, &dir, "// header\n\n").unwrap();

        assert!(!dir.join("src/state").exists());
        let source = fs::read_to_string(dir.join("src/state.rs")).unwrap();
        assert!(source.contains("pub struct State "));
        assert!(source.contains("pub struct Vault "));
        assert!(source.contains("pub fn fetch_state("));
        assert!(source.contains("pub fn fetch_vault("));
        assert_eq!(
            source
                .matches("use crate::sdk_core_offchain::borsh::{BorshDeserialize, BorshSerialize};")
                .count(),
            1
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_state_account_each_account_keeps_its_own_file() {
        let dir = scratch_client("per_account");
        fs::create_dir_all(dir.join("src/state")).unwrap();
        let idl = idl_with_accounts(&["Vault", "GlobalState"]);
        generate_state(&idl, &dir, "// header\n\n").unwrap();

        assert!(!dir.join("src/state.rs").exists());
        assert!(dir.join("src/state/vault.rs").exists());
        assert!(dir.join("src/state/global_state.rs").exists());
        let module = fs::read_to_string(dir.join("src/state/mod.rs")).unwrap();
        assert!(module.contains("pub mod vault;"));
        assert!(module.contains("pub mod global_state;"));
        let _ = fs::remove_dir_all(&dir);
    }
}
