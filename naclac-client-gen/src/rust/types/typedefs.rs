use super::super::{
    is_type_pod, map_type_to_rust_cpi, map_type_to_rust_with_prefix, render_docs, sdk_core_alias,
};
use crate::{Idl, IdlEnumFields, IdlTypeDefVariants};
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
            IdlTypeDefVariants::Enum { variants, repr } => {
                // Always duplicated per offchain/cpi, fieldless or not —
                // matches the struct branch below and, unlike an earlier
                // version of this codegen, avoids two real bugs a "shared
                // body via cfg_attr" form ran into for a *fieldless* enum
                // specifically: (1) a fieldless `#[repr(uN)]` enum is not
                // actually `Pod`-safe (an out-of-range byte value has no
                // corresponding variant — the exact class of bug this whole
                // framework's on-chain `#[defined_type]` macro moved away
                // from; giving the client an `unsafe impl Pod` here would
                // reintroduce it client-side), so it needs the same
                // `CheckedBitPattern` machinery as a data-carrying enum, not
                // a shortcut; (2) that machinery's generated helper type
                // names (e.g. `NameBits`) are derived from the enum's own
                // name, so sharing one enum body between offchain/cpi while
                // still needing two differently-`sdk_core`-qualified copies
                // of the generated helpers would collide under Cargo
                // feature unification (both `offchain` and `cpi` active at
                // once) unless the helper names differ too — simplest to
                // just always duplicate, matching the struct branch's own
                // established convention.
                {
                    let tag_repr = repr.clone().unwrap_or_else(|| "u8".to_string());
                    for for_cpi in [false, true] {
                        let sdk_core = sdk_core_alias(for_cpi);
                        let cfg = if for_cpi {
                            "#[cfg(feature = \"cpi\")]\n"
                        } else {
                            "#[cfg(feature = \"offchain\")]\n"
                        };
                        let enum_name = if for_cpi {
                            format!("{}Cpi", name_camel)
                        } else {
                            name_camel.clone()
                        };

                        // The variant *text* itself, not just the derive
                        // stack above it, has to differ between Borsh and
                        // zero-copy now that a variant can have an explicit
                        // discriminant: zero-copy's `CheckedBitPattern`
                        // machinery needs the real on-chain discriminant
                        // (`v.discriminant`) rendered as `= N`, or the
                        // client's compiler-assigned discriminants silently
                        // diverge from on-chain's (confirmed as a real bug:
                        // wrong bytes on encode, wrong accept/reject on
                        // decode). But real Borsh serialization always uses
                        // positional declaration order, never the actual
                        // Rust discriminant, unless a crate opts into
                        // `#[borsh(use_discriminant = true)]` — which
                        // naclac's on-chain Borsh branch never does — so
                        // writing `= N` into the Borsh copy would be not just
                        // unnecessary but a compile error for any non-unit
                        // variant (`#[repr(inttype)]` is required for that,
                        // and the Borsh branch deliberately has no repr at
                        // all) and would still force an explicit
                        // `use_discriminant` choice besides. So this can no
                        // longer be one shared body with `cfg_attr`-only
                        // derive switching — two full, separately-built
                        // copies, matching the struct branch's own
                        // established per-branch-duplication pattern.
                        let mut borsh_variants = String::new();
                        // Attribute-free `variant_ident = N { field: Type, ... },`
                        // lines — exactly what `syn::parse_str::<syn::ItemEnum>`
                        // below needs to hand real `syn::Variant`s (with real
                        // discriminants) to `checked_enum::generate`. Doc
                        // comments are irrelevant to that call, so
                        // intentionally omitted here even though
                        // `typedefs_content`'s zero-copy copy does include them.
                        let mut zc_variants = String::new();

                        for v in variants {
                            let variant_name = v.name.to_upper_camel_case();
                            let disc = &v.discriminant;
                            let docs = render_docs(&v.docs, "    ");
                            match &v.fields {
                                None => {
                                    borsh_variants
                                        .push_str(&format!("{}    {},\n", docs, variant_name));
                                    zc_variants.push_str(&format!(
                                        "{}    {} = {},\n",
                                        docs, variant_name, disc
                                    ));
                                }
                                Some(IdlEnumFields::Named(fields)) => {
                                    borsh_variants
                                        .push_str(&format!("{}    {} {{\n", docs, variant_name));
                                    zc_variants
                                        .push_str(&format!("{}    {} {{\n", docs, variant_name));
                                    for f in fields {
                                        let f_ty = if for_cpi {
                                            map_type_to_rust_cpi(&f.ty, idl.is_zero_copy, "crate::types::")
                                        } else {
                                            map_type_to_rust_with_prefix(
                                                &f.ty,
                                                idl.is_zero_copy,
                                                "crate::types::",
                                            )
                                        };
                                        let f_snake = AsSnakeCase(&f.name).to_string();
                                        let field_line = format!("        {}: {},\n", f_snake, f_ty);
                                        borsh_variants.push_str(&field_line);
                                        zc_variants.push_str(&field_line);
                                    }
                                    borsh_variants.push_str("    },\n");
                                    zc_variants.push_str(&format!("    }} = {},\n", disc));
                                }
                                Some(IdlEnumFields::Tuple(tys)) => {
                                    let field_tys: Vec<String> = tys
                                        .iter()
                                        .map(|ty| {
                                            if for_cpi {
                                                map_type_to_rust_cpi(ty, idl.is_zero_copy, "crate::types::")
                                            } else {
                                                map_type_to_rust_with_prefix(
                                                    ty,
                                                    idl.is_zero_copy,
                                                    "crate::types::",
                                                )
                                            }
                                        })
                                        .collect();
                                    borsh_variants.push_str(&format!(
                                        "{}    {}({}),\n",
                                        docs,
                                        variant_name,
                                        field_tys.join(", ")
                                    ));
                                    zc_variants.push_str(&format!(
                                        "{}    {}({}) = {},\n",
                                        docs,
                                        variant_name,
                                        field_tys.join(", "),
                                        disc
                                    ));
                                }
                            }
                        }

                        let type_docs = render_docs(&t.docs, "");
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(feature = \"borsh\")]\n");
                        typedefs_content.push_str(&type_docs);
                        typedefs_content.push_str(&format!(
                            "#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]\n\
                             #[borsh(crate = \"{sdk_core}::borsh\")]\n\
                             pub enum {name} {{\n{variants}}}\n\n",
                            sdk_core = sdk_core,
                            name = enum_name,
                            variants = borsh_variants,
                        ));

                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                        typedefs_content.push_str(&type_docs);
                        // Every field of a zero-copy `#[defined_type]` enum is
                        // guaranteed `CheckedBitPattern`-eligible on-chain
                        // already (the macro's own generated impl requires
                        // it, so a non-eligible field wouldn't have compiled
                        // in the first place) — safe to derive `Copy`
                        // unconditionally, mirroring the struct branch's
                        // `is_pod == true` case exactly.
                        typedefs_content.push_str(&format!(
                            "#[derive(Clone, Copy, Debug)]\n\
                             #[repr({tag_repr})]\n\
                             pub enum {name} {{\n{variants}}}\n\n",
                            tag_repr = tag_repr,
                            name = enum_name,
                            variants = zc_variants,
                        ));
                        let variants_src = zc_variants;

                        // Zero-copy mode only: generate the same
                        // `bytemuck::CheckedBitPattern` machinery
                        // `naclac-macros/src/checked_enum.rs` emits
                        // on-chain, so the client can validate/decode a
                        // real instance of this enum from raw bytes with a
                        // layout that's byte-identical to the program's own
                        // — gated `not(feature = "borsh")` since Borsh mode
                        // has no C-layout/Pod concept at all (real
                        // `#[derive(Borsh...)]` above already handles that
                        // path completely).
                        if idl.is_zero_copy {
                            let parsed_enum: syn::ItemEnum = syn::parse_str(&format!(
                                "enum {} {{ {} }}",
                                enum_name, variants_src
                            ))
                            .unwrap_or_else(|e| {
                                panic!(
                                    "defined_type: generated enum `{}` failed to parse as a \
                                     syn::ItemEnum (bug in naclac-client-gen's type-string \
                                     generation, not in the source IDL): {e}",
                                    enum_name
                                )
                            });
                            let ident: syn::Ident = syn::parse_str(&enum_name)
                                .expect("defined_type: enum name must be a valid identifier");
                            let vis: syn::Visibility = syn::parse_quote!(pub);
                            let tag_ty: syn::Type = syn::parse_str(&tag_repr)
                                .expect("defined_type: repr must be a valid Rust integer type");
                            let (extra_items, checked_bit_pattern_impl) =
                                crate::rust::pod_codegen::checked_enum::generate(
                                    &ident,
                                    &vis,
                                    &parsed_enum.variants,
                                    &tag_ty,
                                    sdk_core,
                                );
                            let generated = quote::quote! { #extra_items #checked_bit_pattern_impl };
                            let mut file: syn::File = syn::parse2(generated).expect(
                                "defined_type: checked_enum::generate output must parse as a syn::File",
                            );

                            // `#[cfg(...)]` only gates the single item it's
                            // written directly above — pretty-printing the
                            // whole multi-item block behind one shared `cfg`
                            // line would leave every item after the first
                            // always-compiled regardless of feature flags.
                            // So the two gates this block needs (offchain vs
                            // cpi, and not(borsh)) are attached to every
                            // generated item individually instead.
                            let offchain_cpi_cfg: syn::Attribute = if for_cpi {
                                syn::parse_quote!(#[cfg(feature = "cpi")])
                            } else {
                                syn::parse_quote!(#[cfg(feature = "offchain")])
                            };
                            let not_borsh_cfg: syn::Attribute =
                                syn::parse_quote!(#[cfg(not(feature = "borsh"))]);
                            for item in &mut file.items {
                                let attrs = match item {
                                    syn::Item::Struct(s) => &mut s.attrs,
                                    syn::Item::Union(u) => &mut u.attrs,
                                    syn::Item::Impl(i) => &mut i.attrs,
                                    other => panic!(
                                        "defined_type: checked_enum::generate produced an \
                                         unexpected item kind that this cfg-gating pass doesn't \
                                         handle: {:?}",
                                        other
                                    ),
                                };
                                attrs.insert(0, not_borsh_cfg.clone());
                                attrs.insert(0, offchain_cpi_cfg.clone());
                            }

                            typedefs_content.push_str(&prettyplease::unparse(&file));
                            typedefs_content.push('\n');
                        }
                    }

                    // No plain-name alias to `{name}Cpi` here either, for the
                    // same reason the struct branch below has none: `offchain`
                    // and `cpi` aren't mutually exclusive Cargo features.
                }
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

                // naclac-syn names a tuple struct's fields by their
                // positional index ("0", "1", "2", ...) rather than a real
                // identifier. Those aren't valid Rust field names, so a
                // struct whose fields are named exactly this way must be
                // emitted with tuple-struct syntax (`Foo(pub T0, pub T1);`)
                // instead of the named-field syntax below.
                let is_tuple_struct = !fields.is_empty()
                    && fields.iter().enumerate().all(|(i, f)| f.name == i.to_string());

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

                    let struct_open = if is_tuple_struct { "(" } else { " {" };

                    // Pre-compute each field's real mapped type once — both
                    // the `is_pod`/not-`is_pod` text paths below need it, and
                    // (for `is_pod`) so does the `syn`-based padded-field
                    // reconstruction.
                    let mapped_field_tys: Vec<String> = fields
                        .iter()
                        .map(|f| {
                            if for_cpi {
                                map_type_to_rust_cpi(&f.ty, idl.is_zero_copy, "crate::types::")
                            } else {
                                map_type_to_rust_with_prefix(&f.ty, idl.is_zero_copy, "crate::types::")
                            }
                        })
                        .collect();

                    if !is_pod {
                        // No Pod/padding concept applies at all (a String/Vec/
                        // nested-non-pod field makes bytemuck casting unsafe
                        // regardless) — unchanged from before: one shared body,
                        // Borsh derive only, no repr forcing.
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str(&render_docs(&t.docs, ""));
                        typedefs_content.push_str(&format!(
                            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                             #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                             #[cfg_attr(not(feature = \"borsh\"), derive(Clone, Debug))]\n\
                             pub struct {name}{open}\n",
                            sdk_core = sdk_core,
                            name = struct_name,
                            open = struct_open
                        ));
                        for (f, f_ty) in fields.iter().zip(&mapped_field_tys) {
                            typedefs_content.push_str(&render_docs(&f.docs, "    "));
                            if is_tuple_struct {
                                typedefs_content.push_str(&format!("    pub {},\n", f_ty));
                            } else {
                                let f_snake = AsSnakeCase(&f.name).to_string();
                                typedefs_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
                            }
                        }
                        typedefs_content.push_str(if is_tuple_struct { ");\n\n" } else { "}\n\n" });
                        continue;
                    }

                    // `is_pod`: Borsh mode and zero-copy mode now need
                    // genuinely different field lists (the latter gets real,
                    // verified padding fields spliced in — see below), so
                    // they can no longer share one body via `cfg_attr` the
                    // way the `!is_pod` case above still can. Two fully
                    // separate, mutually-exclusive (`feature = "borsh"` vs
                    // `not(feature = "borsh")`) struct declarations instead.

                    // --- Borsh-mode declaration: original fields, no padding,
                    // no Pod concept at all — matches the on-chain Borsh
                    // branch exactly (`naclac-macros/src/lib.rs`'s `!is_zero_copy`
                    // path is a bare pass-through with zero layout machinery).
                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str("#[cfg(feature = \"borsh\")]\n");
                    typedefs_content.push_str(&render_docs(&t.docs, ""));
                    typedefs_content.push_str(&format!(
                        "#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]\n\
                         #[borsh(crate = \"{sdk_core}::borsh\")]\n\
                         pub struct {name}{open}\n",
                        sdk_core = sdk_core,
                        name = struct_name,
                        open = struct_open
                    ));
                    for (f, f_ty) in fields.iter().zip(&mapped_field_tys) {
                        typedefs_content.push_str(&render_docs(&f.docs, "    "));
                        if is_tuple_struct {
                            typedefs_content.push_str(&format!("    pub {},\n", f_ty));
                        } else {
                            let f_snake = AsSnakeCase(&f.name).to_string();
                            typedefs_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
                        }
                    }
                    typedefs_content.push_str(if is_tuple_struct { ");\n\n" } else { "}\n\n" });

                    // --- Zero-copy-mode declaration: real, verified padding.
                    // Reconstructs the field list as a real `syn::Fields` (via
                    // the same source-string round-trip the enum branch uses)
                    // so the copied `pod_struct_checks::generate` — a
                    // deliberate duplicate of `naclac-macros/src/
                    // pod_struct_checks.rs`, see that file's own doc comment
                    // — can auto-insert the same verified padding fields the
                    // on-chain struct gets, instead of a blind, unverified
                    // `unsafe impl Pod` (the exact class of bug —
                    // `Debouncer`/`EpochTracker` — that file exists to catch).
                    let mut scratch_fields_src = String::new();
                    for (f, f_ty) in fields.iter().zip(&mapped_field_tys) {
                        if is_tuple_struct {
                            scratch_fields_src.push_str(&format!("pub {},\n", f_ty));
                        } else {
                            let f_snake = AsSnakeCase(&f.name).to_string();
                            scratch_fields_src.push_str(&format!("pub {}: {},\n", f_snake, f_ty));
                        }
                    }
                    let scratch_src = if is_tuple_struct {
                        format!("struct {}({});", struct_name, scratch_fields_src)
                    } else {
                        format!("struct {} {{ {} }}", struct_name, scratch_fields_src)
                    };
                    let parsed_struct: syn::ItemStruct = syn::parse_str(&scratch_src)
                        .unwrap_or_else(|e| {
                            panic!(
                                "defined_type: generated struct `{}` failed to parse as a \
                                 syn::ItemStruct (bug in naclac-client-gen's type-string \
                                 generation, not in the source IDL): {e}",
                                struct_name
                            )
                        });
                    let struct_ident: syn::Ident = syn::parse_str(&struct_name)
                        .expect("defined_type: struct name must be a valid identifier");
                    let (padded_fields, extra_items) = crate::rust::pod_codegen::pod_struct_checks::generate(
                        &struct_ident,
                        &parsed_struct.fields,
                        sdk_core,
                    );
                    let semi = if is_tuple_struct { quote::quote! { ; } } else { quote::quote! {} };
                    let struct_item_tokens = quote::quote! {
                        #[derive(Copy, Clone, Debug)]
                        #[repr(C)]
                        pub struct #struct_ident #padded_fields #semi
                    };
                    let struct_file: syn::File = syn::parse2(struct_item_tokens).expect(
                        "defined_type: generated struct item must parse as a syn::File",
                    );
                    let pretty_struct = prettyplease::unparse(&struct_file);

                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    typedefs_content.push_str(&render_docs(&t.docs, ""));
                    typedefs_content.push_str(&pretty_struct);
                    typedefs_content.push('\n');

                    // Same multi-item cfg-gating problem the enum branch
                    // already solves: `#[cfg(...)]` only gates the single
                    // item directly below it, so `extra_items` (the gap
                    // consts, the size assertion, the Pod-bound assertion —
                    // several top-level items) each need both gates attached
                    // individually rather than one shared prefix.
                    let mut extra_file: syn::File = syn::parse2(extra_items).expect(
                        "defined_type: pod_struct_checks::generate output must parse as a syn::File",
                    );
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
                                "defined_type: pod_struct_checks::generate produced an unexpected \
                                 item kind that this cfg-gating pass doesn't handle: {:?}",
                                other
                            ),
                        };
                        attrs.insert(0, not_borsh_cfg.clone());
                        attrs.insert(0, offchain_cpi_cfg.clone());
                    }
                    typedefs_content.push_str(&prettyplease::unparse(&extra_file));
                    typedefs_content.push('\n');

                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    typedefs_content.push_str(&format!(
                        "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
                        sdk_core, struct_name
                    ));
                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    typedefs_content.push_str(&format!(
                        "unsafe impl {}::bytemuck::Pod for {} {{}}\n",
                        sdk_core, struct_name
                    ));
                    // Matches the on-chain macro exactly (`naclac-macros/
                    // src/lib.rs`'s `defined_type` struct branch): not
                    // `#[derive(Default)]` (arrays past length 32 aren't
                    // supported by `std`'s own derive), and needed so a
                    // construction site like `Struct { field: v,
                    // ..Default::default() }` keeps working without the
                    // caller needing to know the auto-inserted padding
                    // field's name.
                    typedefs_content.push_str(cfg);
                    typedefs_content.push_str("#[cfg(not(feature = \"borsh\"))]\n");
                    typedefs_content.push_str(&format!(
                        "impl core::default::Default for {1} {{\n    fn default() -> Self {{\n        {0}::bytemuck::Zeroable::zeroed()\n    }}\n}}\n\n",
                        sdk_core, struct_name
                    ));
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
