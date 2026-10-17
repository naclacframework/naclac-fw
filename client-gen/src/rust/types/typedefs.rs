use {
    super::super::{
        is_type_pod, is_zero_copy_struct_pod, map_type_to_rust_cpi, map_type_to_rust_with_prefix,
        render_docs, sdk_core_alias, zero_copy_gate,
    },
    crate::{Idl, IdlEnumFields, IdlTypeDefVariants},
    heck::{AsSnakeCase, ToUpperCamelCase},
    std::{fs, path::Path},
};

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
                // An enum is never `Pod` (a byte can name no variant); the
                // zero-copy form is stored through `ZcCodec`, which validates.
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

                        // Every variant carries its IDL discriminant as `= N`,
                        // which the zero-copy `ZcCodec` and the Borsh
                        // `use_discriminant` form both read, so client and
                        // program tags cannot differ.
                        let mut zc_variants = String::new();
                        // Wincode's own discriminant, by contrast, is never
                        // read from this native `= N` syntax at all — only
                        // from a `#[wincode(tag = N)]` attribute per variant,
                        // which naclac's on-chain macro emits unconditionally
                        // for the same reason. No `#[repr(...)]` requirement
                        // applies to attributes, unlike native `= N`.
                        let mut wincode_variants = String::new();

                        for v in variants {
                            let variant_name = v.name.to_upper_camel_case();
                            let disc = &v.discriminant;
                            let docs = render_docs(&v.docs, "    ");
                            let wincode_tag = format!("#[wincode(tag = {disc})]\n");
                            match &v.fields {
                                None => {
                                    zc_variants.push_str(&format!(
                                        "{}    {} = {},\n",
                                        docs, variant_name, disc
                                    ));
                                    wincode_variants.push_str(&format!(
                                        "{}    {}    {},\n",
                                        docs, wincode_tag, variant_name
                                    ));
                                }
                                Some(IdlEnumFields::Named(fields)) => {
                                    zc_variants
                                        .push_str(&format!("{}    {} {{\n", docs, variant_name));
                                    wincode_variants.push_str(&format!(
                                        "{}    {}    {} {{\n",
                                        docs, wincode_tag, variant_name
                                    ));
                                    for f in fields {
                                        let f_ty = if for_cpi {
                                            map_type_to_rust_cpi(
                                                &f.ty,
                                                idl.is_zero_copy,
                                                "crate::types::",
                                            )
                                        } else {
                                            map_type_to_rust_with_prefix(
                                                &f.ty,
                                                idl.is_zero_copy,
                                                "crate::types::",
                                            )
                                        };
                                        let f_snake = AsSnakeCase(&f.name).to_string();
                                        let field_line =
                                            format!("        {}: {},\n", f_snake, f_ty);
                                        zc_variants.push_str(&field_line);
                                        wincode_variants.push_str(&field_line);
                                    }
                                    zc_variants.push_str(&format!("    }} = {},\n", disc));
                                    wincode_variants.push_str("    },\n");
                                }
                                Some(IdlEnumFields::Tuple(tys)) => {
                                    let field_tys: Vec<String> = tys
                                        .iter()
                                        .map(|ty| {
                                            if for_cpi {
                                                map_type_to_rust_cpi(
                                                    ty,
                                                    idl.is_zero_copy,
                                                    "crate::types::",
                                                )
                                            } else {
                                                map_type_to_rust_with_prefix(
                                                    ty,
                                                    idl.is_zero_copy,
                                                    "crate::types::",
                                                )
                                            }
                                        })
                                        .collect();
                                    zc_variants.push_str(&format!(
                                        "{}    {}({}) = {},\n",
                                        docs,
                                        variant_name,
                                        field_tys.join(", "),
                                        disc
                                    ));
                                    wincode_variants.push_str(&format!(
                                        "{}    {}    {}({}),\n",
                                        docs,
                                        wincode_tag,
                                        variant_name,
                                        field_tys.join(", ")
                                    ));
                                }
                            }
                        }

                        let type_docs = render_docs(&t.docs, "");
                        // `#[repr({tag_repr})]` here is required by Rust
                        // itself, not a Borsh concept: a data-carrying enum
                        // variant may only have an explicit `= N`
                        // discriminant when the enum has an explicit integer
                        // repr. It has no effect on Borsh's own
                        // serialization, which ignores repr entirely — it
                        // only exists to make the `= N` syntax below legal.
                        // `#[borsh(use_discriminant = true)]` makes Borsh's
                        // own discriminant algorithm read that `= N` instead
                        // of always renumbering positionally, matching
                        // naclac's on-chain `#[defined_type]` macro.
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(feature = \"borsh\")]\n");
                        typedefs_content.push_str(&type_docs);
                        typedefs_content.push_str(&format!(
                            "#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]\n\
                             #[borsh(crate = \"{sdk_core}::borsh\", use_discriminant = true)]\n\
                             #[repr({tag_repr})]\n\
                             pub enum {name} {{\n{variants}}}\n\n",
                            sdk_core = sdk_core,
                            tag_repr = tag_repr,
                            name = enum_name,
                            variants = zc_variants,
                        ));

                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str("#[cfg(feature = \"wincode\")]\n");
                        typedefs_content.push_str(&type_docs);
                        typedefs_content.push_str(&format!(
                            "#[derive(Clone, Debug, {sdk_core}::wincode::SchemaWrite, {sdk_core}::wincode::SchemaRead)]\n\
                             #[wincode(crate = \"{sdk_core}::wincode\")]\n\
                             pub enum {name} {{\n{variants}}}\n\n",
                            sdk_core = sdk_core,
                            name = enum_name,
                            variants = wincode_variants,
                        ));

                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str(
                            "#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n",
                        );
                        typedefs_content.push_str(&type_docs);
                        typedefs_content.push_str(&format!(
                            "#[derive(Clone, Copy, Debug)]\n\
                             #[repr({tag_repr})]\n\
                             pub enum {name} {{\n{variants}}}\n\n",
                            tag_repr = tag_repr,
                            name = enum_name,
                            variants = zc_variants,
                        ));
                        let variants_src = zc_variants;

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
                            let tag_ty: syn::Type = syn::parse_str(&tag_repr)
                                .expect("defined_type: repr must be a valid Rust integer type");
                            let core: syn::Path = syn::parse_str(sdk_core)
                                .expect("defined_type: sdk_core alias must be a valid path");
                            let generated = naclac_syn::zc_codegen::enum_codec::generate(
                                &ident,
                                &parsed_enum.variants,
                                &tag_ty,
                                &zero_copy_gate(for_cpi),
                                &core,
                            );
                            let file: syn::File = syn::parse2(generated).expect(
                                "defined_type: enum_codec::generate output must parse as a syn::File",
                            );

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
                let is_pod = fields.iter().all(|f| {
                    if idl.is_zero_copy {
                        is_zero_copy_struct_pod(&f.ty, &idl.defined_types)
                    } else {
                        is_type_pod(&f.ty, &idl.defined_types)
                    }
                });

                // naclac-syn names a tuple struct's fields by their
                // positional index ("0", "1", "2", ...) rather than a real
                // identifier. Those aren't valid Rust field names, so a
                // struct whose fields are named exactly this way must be
                // emitted with tuple-struct syntax (`Foo(pub T0, pub T1);`)
                // instead of the named-field syntax below.
                let is_tuple_struct = !fields.is_empty()
                    && fields
                        .iter()
                        .enumerate()
                        .all(|(i, f)| f.name == i.to_string());

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
                                map_type_to_rust_with_prefix(
                                    &f.ty,
                                    idl.is_zero_copy,
                                    "crate::types::",
                                )
                            }
                        })
                        .collect();

                    if !is_pod {
                        typedefs_content.push_str(cfg);
                        typedefs_content.push_str(&render_docs(&t.docs, ""));
                        typedefs_content.push_str(&format!(
                            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                             #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                             #[cfg_attr(feature = \"wincode\", derive(Clone, Debug, {sdk_core}::wincode::SchemaWrite, {sdk_core}::wincode::SchemaRead))]\n\
                             #[cfg_attr(feature = \"wincode\", wincode(crate = \"{sdk_core}::wincode\"))]\n\
                             #[cfg_attr(not(any(feature = \"borsh\", feature = \"wincode\")), derive(Clone, Debug))]\n\
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
                                typedefs_content
                                    .push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
                            }
                        }
                        typedefs_content.push_str(if is_tuple_struct { ");\n\n" } else { "}\n\n" });
                        continue;
                    }

                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(any(feature = \"borsh\", feature = \"wincode\"))]\n");
                    typedefs_content.push_str(&render_docs(&t.docs, ""));
                    typedefs_content.push_str(&format!(
                        "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
                         #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
                         #[cfg_attr(feature = \"wincode\", derive(Clone, Debug, {sdk_core}::wincode::SchemaWrite, {sdk_core}::wincode::SchemaRead))]\n\
                         #[cfg_attr(feature = \"wincode\", wincode(crate = \"{sdk_core}::wincode\"))]\n\
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

                    let zero_copy_field_tys: Vec<String> = fields
                        .iter()
                        .map(|f| {
                            crate::rust::map_zero_copy_field_type(
                                &f.ty,
                                "crate::types::",
                                for_cpi,
                                &idl.defined_types,
                            )
                        })
                        .collect();
                    let mut scratch_fields_src = String::new();
                    for (f, f_ty) in fields.iter().zip(&zero_copy_field_tys) {
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
                    let core: syn::Path = syn::parse_str(sdk_core)
                        .expect("defined_type: sdk_core alias must be a valid path");
                    let layout_checks = naclac_syn::zc_codegen::struct_checks::generate(
                        &struct_ident,
                        &parsed_struct.fields,
                        &zero_copy_gate(for_cpi),
                        &core,
                    );
                    let struct_fields = &parsed_struct.fields;
                    let semi = if is_tuple_struct {
                        quote::quote! { ; }
                    } else {
                        quote::quote! {}
                    };
                    let struct_item_tokens = quote::quote! {
                        #[derive(Copy, Clone, Debug)]
                        #[repr(C, packed)]
                        pub struct #struct_ident #struct_fields #semi
                    };
                    let struct_file: syn::File = syn::parse2(struct_item_tokens)
                        .expect("defined_type: generated struct item must parse as a syn::File");
                    let pretty_struct = prettyplease::unparse(&struct_file);

                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
                    typedefs_content.push_str(&render_docs(&t.docs, ""));
                    typedefs_content.push_str(&pretty_struct);
                    typedefs_content.push('\n');

                    let layout_file: syn::File = syn::parse2(layout_checks).expect(
                        "defined_type: struct_checks::generate output must parse as a syn::File",
                    );
                    typedefs_content.push_str(&prettyplease::unparse(&layout_file));
                    typedefs_content.push('\n');

                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
                    typedefs_content.push_str(&format!(
                        "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
                        sdk_core, struct_name
                    ));
                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
                    typedefs_content.push_str(&format!(
                        "unsafe impl {}::bytemuck::Pod for {} {{}}\n",
                        sdk_core, struct_name
                    ));
                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
                    typedefs_content.push_str(&format!(
                        "impl {0}::ZcCodec for {1} {{\n    const SIZE: usize = core::mem::size_of::<{1}>();\n    fn encode(&self, out: &mut [u8]) {{\n        {0}::encode_pod(self, out);\n    }}\n    fn decode(bytes: &[u8]) -> Option<Self> {{\n        {0}::decode_pod(bytes)\n    }}\n}}\n\n",
                        sdk_core, struct_name
                    ));
                    typedefs_content.push_str(cfg);
                    typedefs_content
                        .push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
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
