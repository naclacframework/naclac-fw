use super::{
    map_type_to_rust_cpi, map_type_to_rust_with_prefix, references_defined_type, render_docs,
    sdk_core_alias,
};
use crate::Idl;
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate_components(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.accounts.is_empty() {
        return Ok(());
    }

    let mut components_mod = header.to_string();
    for acc in &idl.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        components_mod.push_str(&format!("pub mod {};\n", acc_snake));
        components_mod.push_str(&format!("pub use {}::*;\n", acc_snake));
    }
    fs::write(clients_dir.join("src/components/mod.rs"), components_mod).unwrap();

    for acc in &idl.accounts {
        let acc_snake = AsSnakeCase(&acc.name).to_string();
        let acc_camel = acc.name.to_upper_camel_case();

        let mut comp_content = header.to_string();
        comp_content.push_str(
            "#[cfg(all(feature = \"borsh\", feature = \"offchain\"))]\n\
             use crate::sdk_core_offchain::borsh::{BorshDeserialize, BorshSerialize};\n\
             #[cfg(all(feature = \"borsh\", not(feature = \"offchain\")))]\n\
             use crate::sdk_core_cpi::borsh::{BorshDeserialize, BorshSerialize};\n",
        );
        if acc.ty.fields.iter().any(|f| references_defined_type(&f.ty)) {
            comp_content.push_str("use crate::types::typedefs::*;\n");
        }
        comp_content.push('\n');

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

            // Borsh mode and zero-copy mode can no longer share one body via
            // `cfg_attr` — the zero-copy declaration below gets real,
            // verified padding fields spliced in (via the same copied
            // `pod_struct_checks::generate` `types/typedefs.rs`'s struct
            // branch uses), which the Borsh derive must never see (Borsh has
            // no padding, ever). Two fully separate, mutually-exclusive
            // (`feature = "borsh"` vs `not(feature = "borsh")`) struct
            // declarations instead — same rationale as `types/typedefs.rs`.

            // --- Borsh-mode declaration: original fields, no padding, no
            // Pod concept at all.
            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(feature = \"borsh\")]\n");
            comp_content.push_str(&render_docs(&acc.docs, ""));
            comp_content.push_str(&format!(
                "#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]\n\
                 #[borsh(crate = \"{sdk_core}::borsh\")]\n\
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

            // --- Zero-copy-mode declaration: real, verified padding. Every
            // `#[component]` zero-copy account is assumed Pod-eligible by
            // construction (on-chain, the macro's own real `#[derive(Pod,
            // Zeroable)]` on the struct enforces this at compile time — a
            // heap-owning/dynamic field simply couldn't compile there), so
            // — unlike `types/typedefs.rs`'s struct branch — there's no
            // `is_pod` check needed here; this path is unconditional.
            let mut scratch_fields_src = String::new();
            for (field, field_ty) in acc.ty.fields.iter().zip(&mapped_field_tys) {
                let field_snake = AsSnakeCase(&field.name).to_string();
                scratch_fields_src.push_str(&format!("pub {}: {},\n", field_snake, field_ty));
            }
            let scratch_src = format!("struct {} {{ {} }}", struct_name, scratch_fields_src);
            let parsed_struct: syn::ItemStruct = syn::parse_str(&scratch_src).unwrap_or_else(|e| {
                panic!(
                    "component: generated account struct `{}` failed to parse as a \
                     syn::ItemStruct (bug in naclac-client-gen's type-string generation, \
                     not in the source IDL): {e}",
                    struct_name
                )
            });
            let struct_ident: syn::Ident = syn::parse_str(&struct_name)
                .expect("component: account struct name must be a valid identifier");
            let (padded_fields, extra_items) = crate::rust::pod_codegen::pod_struct_checks::generate(
                &struct_ident,
                &parsed_struct.fields,
                sdk_core,
            );
            let struct_item_tokens = quote::quote! {
                #[derive(Copy, Clone, Debug)]
                #[repr(C)]
                pub struct #struct_ident #padded_fields
            };
            let struct_file: syn::File = syn::parse2(struct_item_tokens)
                .expect("component: generated account struct item must parse as a syn::File");

            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
            comp_content.push_str(&render_docs(&acc.docs, ""));
            comp_content.push_str(&prettyplease::unparse(&struct_file));
            comp_content.push('\n');

            // Same multi-item cfg-gating requirement `types/typedefs.rs`'s
            // struct branch already documents: `#[cfg(...)]` only gates the
            // single item directly below it, so each of `extra_items`'
            // several top-level `const` items needs both gates attached
            // individually.
            let mut extra_file: syn::File = syn::parse2(extra_items)
                .expect("component: pod_struct_checks::generate output must parse as a syn::File");
            let offchain_cpi_cfg: syn::Attribute = if for_cpi {
                syn::parse_quote!(#[cfg(feature = "cpi")])
            } else {
                syn::parse_quote!(#[cfg(feature = "offchain")])
            };
            let not_borsh_cfg: syn::Attribute = syn::parse_quote!(#[cfg(not(feature = "borsh"))]);
            for item in &mut extra_file.items {
                let attrs = match item {
                    syn::Item::Const(c) => &mut c.attrs,
                    syn::Item::Fn(f) => &mut f.attrs,
                    other => panic!(
                        "component: pod_struct_checks::generate produced an unexpected item \
                         kind that this cfg-gating pass doesn't handle: {:?}",
                        other
                    ),
                };
                attrs.insert(0, not_borsh_cfg.clone());
                attrs.insert(0, offchain_cpi_cfg.clone());
            }
            comp_content.push_str(&prettyplease::unparse(&extra_file));
            comp_content.push('\n');

            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
            comp_content.push_str(&format!(
                "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
                sdk_core, struct_name
            ));
            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
            comp_content.push_str(&format!(
                "unsafe impl {}::bytemuck::Pod for {} {{}}\n",
                sdk_core, struct_name
            ));
            // Matches `types/typedefs.rs`'s struct branch and the real
            // on-chain `#[component]` macro (`naclac-macros/src/
            // component.rs`) — needed so a construction site like
            // `Struct { field: v, ..Default::default() }` keeps working
            // without knowing the auto-inserted padding field's name.
            comp_content.push_str(cfg);
            comp_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
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
                "\n/// 8-byte on-chain discriminator for `{}` accounts.\n\
                 pub const {}_DISCRIMINATOR: [u8; 8] = [{}];\n",
                acc_camel, disc_name, disc_bytes,
            ));

            // A single `AccountFetcher::fetch::<{acc_camel}>()` call site works
            // correctly regardless of Borsh vs zero-copy mode: the decode
            // strategy is picked here, at THIS type's own compile time (via the
            // generated client crate's own `borsh` feature, set correctly from
            // the real IDL at generation time), not guessed at runtime.
            comp_content.push_str(&format!(
                r#"#[cfg(feature = "offchain")]
#[cfg(not(feature = "borsh"))]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<[u8; 8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_pod_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
    }}
}}

#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
impl naclac_client::NaclacDecode for {acc_camel} {{
    const DISCRIMINATOR: Option<[u8; 8]> = Some({disc_name}_DISCRIMINATOR);
    fn naclac_decode(data: &[u8]) -> Result<Self, naclac_client::NaclacClientError> {{
        naclac_client::decode_borsh_checked::<Self>(data, {disc_name}_DISCRIMINATOR)
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
    let raw_accounts = provider.get_program_accounts(program_id, Some(&{disc_name}_DISCRIMINATOR))?;
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

        fs::write(
            clients_dir.join(format!("src/components/{}.rs", acc_snake)),
            comp_content,
        )
        .unwrap();
    }

    Ok(())
}
