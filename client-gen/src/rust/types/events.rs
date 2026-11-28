use {
    super::super::{
        map_type_to_rust_cpi, map_type_to_rust_with_prefix, render_docs, sdk_core_alias,
    },
    crate::{Idl, IdlEventDef},
    heck::{AsSnakeCase, ToUpperCamelCase},
    std::{fs, path::Path},
};

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.events.is_empty() {
        return Ok(());
    }

    let mut events_content = header.to_string();
    // `offchain`/`cpi` aren't mutually exclusive at the Cargo level (feature
    // unification can activate both in one build), so the borsh-impl import
    // pair below splits on `feature = "offchain"` vs `not(feature =
    // "offchain")` — safe here because both arms import the same two trait
    // *names* (`BorshDeserialize`/`BorshSerialize`) from whichever crate
    // path is live, not a type whose shape differs per branch. The event
    // structs themselves are a different story (see `generate_fixed_event`):
    // those get independent `X`/`XCpi` names, mirroring
    // `state.rs`/`types/typedefs.rs`.
    events_content.push_str(
        "#[cfg(all(feature = \"borsh\", feature = \"offchain\"))]\n\
         use crate::sdk_core_offchain::borsh::{BorshDeserialize, BorshSerialize};\n\
         #[cfg(all(feature = \"borsh\", not(feature = \"offchain\")))]\n\
         use crate::sdk_core_cpi::borsh::{BorshDeserialize, BorshSerialize};\n",
    );
    let needs_typedefs = idl.events.iter().any(|e| {
        e.fields
            .iter()
            .any(|f| super::super::references_defined_type(&f.ty))
    });
    if needs_typedefs {
        events_content.push_str("use crate::types::typedefs::*;\n");
    }
    events_content.push('\n');

    for event in &idl.events {
        let event_camel = event.name.to_upper_camel_case();
        generate_fixed_event(
            &mut events_content,
            event,
            &event_camel,
            idl.is_zero_copy,
            &idl.defined_types,
        );
        generate_event_discriminator_impl(
            &mut events_content,
            event,
            &event_camel,
            idl.event_cpi_tag.as_deref(),
        );
    }

    fs::write(clients_dir.join("src/types/events.rs"), events_content).unwrap();
    Ok(())
}

/// Emits `impl NaclacEvent for <Event>` carrying the event's IDL discriminator,
/// the emitting program's ID, and its self-CPI tag (empty when the program
/// never uses `#[event_cpi]`). Only the off-chain struct gets it:
/// `NaclacEvent` is how `parse_events_*` finds an event in a transaction's
/// logs and inner instructions, which only the off-chain client does.
fn generate_event_discriminator_impl(
    events_content: &mut String,
    event: &IdlEventDef,
    event_camel: &str,
    event_cpi_tag: Option<&[u8]>,
) {
    let Some(discriminator) = &event.discriminator else {
        return;
    };
    let join = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    events_content.push_str("#[cfg(feature = \"offchain\")]\n");
    events_content.push_str(&format!(
        "impl crate::sdk_core_offchain::NaclacEvent for {} {{\n    const DISCRIMINATOR: &'static [u8] = &[{}];\n    const PROGRAM_ID: crate::sdk_core_offchain::Address = crate::types::PROGRAM_ID;\n    const CPI_TAG: &'static [u8] = &[{}];\n}}\n\n",
        event_camel,
        join(discriminator),
        join(event_cpi_tag.unwrap_or(&[]))
    ));
}

/// Plain `#[event]` — fixed-size, `bytemuck`-cast wire format.
fn generate_fixed_event(
    events_content: &mut String,
    event: &IdlEventDef,
    event_camel: &str,
    is_zero_copy: bool,
    defined_types: &[crate::IdlType],
) {
    // Field types (e.g. `Address`/`Bool`) genuinely differ between
    // `sdk_core_offchain` and `sdk_core_cpi` — not interchangeable, just
    // same-shaped — so the whole struct must be duplicated per branch
    // rather than sharing one body. Gated on `feature = "cpi"` / `feature =
    // "offchain"` independently (not a mutually-exclusive `offchain`/
    // `not(offchain)` pair) so both can coexist under their own distinct
    // name when a crate is reached both ways at once (mirrors
    // `state.rs`/`types/typedefs.rs`).
    for for_cpi in [false, true] {
        let sdk_core = sdk_core_alias(for_cpi);
        let cfg = if for_cpi {
            "#[cfg(feature = \"cpi\")]\n"
        } else {
            "#[cfg(feature = \"offchain\")]\n"
        };
        let struct_name = if for_cpi {
            format!("{}Cpi", event_camel)
        } else {
            event_camel.to_string()
        };

        let mapped_field_tys: Vec<String> = event
            .fields
            .iter()
            .map(|field| {
                if for_cpi {
                    map_type_to_rust_cpi(&field.ty, is_zero_copy, "")
                } else {
                    map_type_to_rust_with_prefix(&field.ty, is_zero_copy, "")
                }
            })
            .collect();

        // Borsh / Wincode: the fields as written, tightly packed.
        events_content.push_str(cfg);
        events_content.push_str("#[cfg(any(feature = \"borsh\", feature = \"wincode\"))]\n");
        events_content.push_str(&render_docs(&event.docs, ""));
        events_content.push_str(&format!(
            "#[cfg_attr(feature = \"borsh\", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]\n\
             #[cfg_attr(feature = \"borsh\", borsh(crate = \"{sdk_core}::borsh\"))]\n\
             #[cfg_attr(feature = \"wincode\", derive(Clone, Debug, {sdk_core}::wincode::SchemaWrite, {sdk_core}::wincode::SchemaRead))]\n\
             #[cfg_attr(feature = \"wincode\", wincode(crate = \"{sdk_core}::wincode\"))]\n\
             pub struct {name} {{\n",
            sdk_core = sdk_core,
            name = struct_name
        ));
        for (field, f_ty) in event.fields.iter().zip(&mapped_field_tys) {
            let f_snake = AsSnakeCase(&field.name).to_string();
            events_content.push_str(&render_docs(&field.docs, "    "));
            events_content.push_str(&format!("    pub {}: {},\n", f_snake, f_ty));
        }
        events_content.push_str("}\n\n");

        let zero_copy_field_tys: Vec<String> = event
            .fields
            .iter()
            .map(|field| {
                crate::rust::map_zero_copy_field_type(&field.ty, "", for_cpi, defined_types)
            })
            .collect();
        let mut scratch_fields_src = String::new();
        for (field, f_ty) in event.fields.iter().zip(&zero_copy_field_tys) {
            let f_snake = AsSnakeCase(&field.name).to_string();
            scratch_fields_src.push_str(&format!("pub {}: {},\n", f_snake, f_ty));
        }
        let scratch_src = format!("struct {} {{ {} }}", struct_name, scratch_fields_src);
        let parsed_struct: syn::ItemStruct = syn::parse_str(&scratch_src).unwrap_or_else(|e| {
            panic!(
                "event: generated struct `{}` failed to parse as a syn::ItemStruct (bug in \
                 naclac-client-gen's type-string generation, not in the source IDL): {e}",
                struct_name
            )
        });
        let struct_ident: syn::Ident =
            syn::parse_str(&struct_name).expect("event: struct name must be a valid identifier");
        let core: syn::Path =
            syn::parse_str(sdk_core).expect("event: sdk_core alias must be a valid path");
        let layout_checks = naclac_syn::zc_codegen::struct_checks::generate(
            &struct_ident,
            &parsed_struct.fields,
            &crate::rust::zero_copy_gate(for_cpi),
            &core,
        );
        let struct_fields = &parsed_struct.fields;
        let struct_file: syn::File = syn::parse2(quote::quote! {
            #[derive(Copy, Clone, Debug)]
            #[repr(C, packed)]
            pub struct #struct_ident #struct_fields
        })
        .expect("event: generated struct item must parse as a syn::File");
        events_content.push_str(cfg);
        events_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
        events_content.push_str(&render_docs(&event.docs, ""));
        events_content.push_str(&prettyplease::unparse(&struct_file));
        events_content.push('\n');

        let layout_file: syn::File = syn::parse2(layout_checks)
            .expect("event: struct_checks::generate output must parse as a syn::File");
        events_content.push_str(&prettyplease::unparse(&layout_file));
        events_content.push('\n');

        events_content.push_str(cfg);
        events_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
        events_content.push_str(&format!(
            "unsafe impl {}::bytemuck::Zeroable for {} {{}}\n",
            sdk_core, struct_name
        ));
        events_content.push_str(cfg);
        events_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
        events_content.push_str(&format!(
            "unsafe impl {}::bytemuck::Pod for {} {{}}\n\n",
            sdk_core, struct_name
        ));

        let validate_impl = naclac_syn::zc_codegen::pod_validate::generate(
            &struct_ident,
            &parsed_struct.fields,
            &quote::quote! {},
            &core,
        );
        let validate_file: syn::File = syn::parse2(validate_impl)
            .expect("event: pod_validate::generate output must parse as a syn::File");
        events_content.push_str(cfg);
        events_content.push_str("#[cfg(not(any(feature = \"borsh\", feature = \"wincode\")))]\n");
        events_content.push_str(&prettyplease::unparse(&validate_file));
        events_content.push('\n');
    }
}
