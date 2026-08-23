use super::super::{
    is_type_pod, map_type_to_rust_cpi, map_type_to_rust_with_prefix, render_docs, sdk_core_alias,
};
use crate::{Idl, IdlTypeDefVariants};
use heck::{AsSnakeCase, ToUpperCamelCase};
use std::fs;
use std::path::Path;

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.defined_types.is_empty() {
        return Ok(());
    }

    let mut typedefs_content = header.to_string();
    // These two `BorshSerialize`/`BorshDeserialize` imports (unlike the
    // per-type structs generated below) share one name each regardless of
    // offchain/cpi, so they're kept mutually exclusive (`offchain` vs
    // `not(offchain)`, matching `types/constants.rs`'s `PROGRAM_ID`/
    // `publicKey`-typed constants) rather than independently gated — the
    // same rule the per-type structs below deliberately do NOT follow, since
    // those get a distinct name per branch instead (see that loop's own
    // comment).
    typedefs_content.push_str(
        "#[cfg(all(feature = \"borsh\", feature = \"offchain\"))]\n\
         use crate::sdk_core_offchain::borsh::{BorshDeserialize, BorshSerialize};\n\
         #[cfg(all(feature = \"borsh\", not(feature = \"offchain\")))]\n\
         use crate::sdk_core_cpi::borsh::{BorshDeserialize, BorshSerialize};\n\n",
    );

    for t in &idl.defined_types {
        let name_camel = t.name.to_upper_camel_case();
        match &t.ty {
            IdlTypeDefVariants::Enum { variants } => {
                // Body is identical regardless of offchain/cpi — only the
                // `borsh` derive's crate path depends on which alias is
                // active, so stack `cfg_attr`s instead of duplicating the
                // whole `pub enum` (which would need two different names to
                // avoid colliding when both features are unified together).
                typedefs_content.push_str(&render_docs(&t.docs, ""));
                typedefs_content.push_str(&format!(
                    "#[cfg_attr(all(feature = \"borsh\", feature = \"offchain\"), derive(BorshSerialize, BorshDeserialize))]\n\
                     #[cfg_attr(all(feature = \"borsh\", feature = \"offchain\"), borsh(crate = \"crate::sdk_core_offchain::borsh\"))]\n\
                     #[cfg_attr(all(feature = \"borsh\", not(feature = \"offchain\")), derive(BorshSerialize, BorshDeserialize))]\n\
                     #[cfg_attr(all(feature = \"borsh\", not(feature = \"offchain\")), borsh(crate = \"crate::sdk_core_cpi::borsh\"))]\n\
                     #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
                     #[repr(u8)]\n\
                     pub enum {} {{\n",
                    name_camel
                ));
                for v in variants {
                    typedefs_content.push_str(&render_docs(&v.docs, "    "));
                    typedefs_content.push_str(&format!("    {},\n", v.name.to_upper_camel_case()));
                }
                typedefs_content.push_str("}\n\n");

                // `map_type_to_rust_inner` always appends a `Cpi` suffix to a
                // `Defined` reference under `for_cpi` (needed for `Struct`
                // variants — see that function's own comment), regardless of
                // whether the referenced type is this enum or a struct; this
                // alias keeps that reference resolving correctly without
                // needing the type-mapping functions to look up whether a
                // given name is an enum or a struct.
                typedefs_content.push_str(&format!("pub type {0}Cpi = {0};\n\n", name_camel));
            }
            IdlTypeDefVariants::Struct { fields } => {
                // A field containing a `String`/`Vec<T>` (or, in zero-copy
                // mode, a `ZcString`/`ZcVec<T>`) makes the whole struct unsafe
                // to derive `Copy`/`bytemuck::Pod` for — those types own a
                // heap allocation or are a zero-copy *view* (pointer +
                // length) into someone else's buffer, neither of which is a
                // plain, arbitrary-bytes-safe in-place representation. Same
                // check as `naclac-macros`'s `#[instruction_args]` uses for
                // the on-chain struct this type mirrors.
                let is_pod = fields
                    .iter()
                    .all(|f| is_type_pod(&f.ty, &idl.defined_types));

                // Field types (e.g. `Address`/`Bool`) genuinely differ between
                // `sdk_core_offchain` and `sdk_core_cpi` — not interchangeable,
                // just same-shaped — so, unlike the enum case above, the whole
                // struct (and its Zeroable/Pod impls) must be duplicated per
                // branch rather than sharing one body. Gated on `feature =
                // "offchain"` / `feature = "cpi"` independently (not a
                // mutually-exclusive `offchain`/`not(offchain)` pair) so both
                // can coexist under their own distinct name when a crate is
                // reached both ways at once (e.g. a normal `cpi`-only
                // dependency and an `offchain`-only dev-dependency on the
                // same package, unifying both features) — the same reason
                // `instructions/mod.rs` gates `XxxIxArgs`/`XxxCpiIxArgs` this
                // way instead of sharing one name.
                for for_cpi in [false, true] {
                    let sdk_core = sdk_core_alias(for_cpi);
                    let cfg = if for_cpi {
                        "#[cfg(feature = \"cpi\")]\n"
                    } else {
                        "#[cfg(feature = \"offchain\")]\n"
                    };
                    // Distinct names, not just distinct cfg gates — see
                    // `map_type_to_rust_inner`'s comment on the matching
                    // `Cpi`-suffix it appends for every reference to this type.
                    let struct_name = if for_cpi {
                        format!("{}Cpi", name_camel)
                    } else {
                        name_camel.clone()
                    };

                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str(&render_docs(&t.docs, ""));
                    if is_pod {
                        typedefs_content.push_str(&format!(
                            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                             #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                             #[cfg_attr(not(feature = \"borsh\"), derive(Copy, Clone, Debug))]\n\
                             #[cfg_attr(not(feature = \"borsh\"), repr(C))]\n\
                             pub struct {name} {{\n",
                            sdk_core = sdk_core,
                            name = struct_name
                        ));
                    } else {
                        typedefs_content.push_str(&format!(
                            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                             #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                             #[cfg_attr(not(feature = \"borsh\"), derive(Clone, Debug))]\n\
                             pub struct {name} {{\n",
                            sdk_core = sdk_core,
                            name = struct_name
                        ));
                    }

                    for f in fields {
                        let f_snake = AsSnakeCase(&f.name).to_string();
                        let f_ty = if for_cpi {
                            map_type_to_rust_cpi(&f.ty, idl.is_zero_copy, "crate::types::")
                        } else {
                            map_type_to_rust_with_prefix(&f.ty, idl.is_zero_copy, "crate::types::")
                        };
                        typedefs_content.push_str(&render_docs(&f.docs, "    "));
                        typedefs_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
                    }
                    typedefs_content.push_str("}\n\n");

                    if is_pod {
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                        typedefs_content.push_str(&format!(
                            "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
                            sdk_core, struct_name
                        ));
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                        typedefs_content.push_str(&format!(
                            "unsafe impl {}::bytemuck::Pod for {} {{}}\n\n",
                            sdk_core, struct_name
                        ));
                    }
                }

                // No alias from the plain name to `{name}Cpi`: `offchain` and
                // `cpi` aren't mutually exclusive at the Cargo level (feature
                // unification can activate both in one build), and an alias
                // gated on `not(feature = "offchain")` would silently vanish
                // the moment offchain is also active, leaving the plain name
                // pointing at the *offchain* struct instead -- exactly the
                // ambiguity this file's independent per-branch naming exists
                // to avoid (see the loop above). Every on-chain CPI caller
                // must reference the `Cpi`-suffixed name explicitly, the same
                // way `instructions/mod.rs` requires `{Ix}CpiIxArgs` rather
                // than a bare `{Ix}IxArgs` alias.
            }
        }
    }
    fs::write(clients_dir.join("src/types/typedefs.rs"), typedefs_content).unwrap();

    Ok(())
}
