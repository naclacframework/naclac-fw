use crate::rust::{collect_defined_type_names, references_defined_type};
use crate::{Idl, IdlTypeDefVariants};
use heck::{ToLowerCamelCase, ToUpperCamelCase};
use std::fs;

// ─── Type Mapping ─────────────────────────────────────────────────────────────────────────────────

/// Maps an IDL type (as serde_json::Value) to a TypeScript type string.
/// `is_zero_copy` determines whether Pod-specific wrappers (e.g. `naclac.Bool`) are used.
pub fn map_type_to_ts_with_prefix(
    idl_type: &serde_json::Value,
    is_zero_copy: bool,
    defined_prefix: &str,
) -> String {
    match idl_type {
        serde_json::Value::String(s) => match s.as_str() {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "f32" | "f64" | "usize" | "isize" => {
                "number".to_string()
            }
            "u64" | "u128" | "i64" | "i128" => "bigint | number".to_string(),
            "bool" => "boolean".to_string(),
            "string" | "String" => "string".to_string(),
            "publicKey" => "naclac.Address | string".to_string(),
            "bytes" | "&[u8]" | "[u8]" => "Uint8Array".to_string(),
            "Bool" if is_zero_copy => "naclac.Bool".to_string(),
            _ => {
                if is_zero_copy && s.starts_with("Opt") {
                    return format!("naclac.{}", s);
                }
                if !defined_prefix.is_empty() {
                    format!("{}{}", defined_prefix, s)
                } else {
                    s.to_string()
                }
            }
        },
        serde_json::Value::Object(o) => {
            if let Some(inner) = o.get("option") {
                return format!(
                    "{} | null",
                    map_type_to_ts_with_prefix(inner, is_zero_copy, defined_prefix)
                );
            }
            if let Some(inner) = o.get("vec") {
                if inner == &serde_json::Value::String("u8".to_string()) {
                    return "Uint8Array".to_string();
                }
                return format!(
                    "Array<{}>",
                    map_type_to_ts_with_prefix(inner, is_zero_copy, defined_prefix)
                );
            }
            if let Some(arr) = o.get("array") {
                if let Some(inner) = arr.get(0) {
                    if inner == &serde_json::Value::String("u8".to_string()) {
                        return "string | Uint8Array".to_string();
                    }
                    return format!(
                        "Array<{}>",
                        map_type_to_ts_with_prefix(inner, is_zero_copy, defined_prefix)
                    );
                }
            }
            if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                if is_zero_copy && (defined == "Bool" || defined.starts_with("Opt")) {
                    return format!("naclac.{}", defined);
                }
                if !defined_prefix.is_empty() {
                    return format!("{}{}", defined_prefix, defined);
                }
                return defined.to_string();
            }
            "any".to_string()
        }
        _ => "any".to_string(),
    }
}

pub fn map_type_to_ts(idl_type: &serde_json::Value, is_zero_copy: bool) -> String {
    map_type_to_ts_with_prefix(idl_type, is_zero_copy, "")
}

/// Renders IDL `docs` lines as a JSDoc comment block, prefixed with `indent`.
/// A single line collapses to `/** line */`; multiple lines use a full
/// `/** \n * ... \n */` block. Returns an empty string when there are no docs.
pub fn render_docs_ts(docs: &[String], indent: &str) -> String {
    match docs {
        [] => String::new(),
        [line] => format!("{}/** {} */\n", indent, line),
        lines => {
            let mut out = format!("{}/**\n", indent);
            for line in lines {
                out.push_str(&format!("{} * {}\n", indent, line));
            }
            out.push_str(&format!("{} */\n", indent));
            out
        }
    }
}

/// Formats a constant value string for TypeScript output.
pub fn format_const_value(val_str: &str, ty_str: &str) -> String {
    let val = val_str.trim();

    // Handle decimal byte-array values (`bytes`- or `{"array": ["u8", N]}`-typed constants)
    if val.starts_with('[') && val.ends_with(']') {
        return format!("Uint8Array.from({})", val);
    }

    if ty_str == "publicKey" {
        return format!("naclac.address(\"{}\")", val);
    }

    // Handle bigints
    if ty_str == "u64" || ty_str == "u128" || ty_str == "i64" || ty_str == "i128" {
        let clean = val
            .trim_end_matches("u64")
            .trim_end_matches("u128")
            .trim_end_matches("i64")
            .trim_end_matches("i128")
            .trim_end_matches("usize")
            .trim_end_matches("isize");
        if let Ok(n) = clean.parse::<u64>() {
            return format!("{}n", n);
        }
    }

    // Handle usize/isize
    if ty_str == "usize" || ty_str == "isize" {
        return val
            .trim_end_matches("usize")
            .trim_end_matches("isize")
            .to_string();
    }

    val.to_string()
}

/// Generates the TypeScript IDL export file (idl/<name>.ts).
pub fn generate_ts(idl_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let parsed: serde_json::Value = serde_json::from_str(idl_json)?;
    let program_name = parsed
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("UnknownProgram")
        .to_string();

    let type_name = program_name.to_upper_camel_case();
    let ts_content = format!(
        "export const IDL = {} as const;\n\nexport type {} = typeof IDL;\n",
        idl_json, type_name
    );

    Ok(ts_content)
}

// ─── Shared File Generator ────────────────────────────────────────────────────────────────────────

pub fn generate_shared_files(
    idl: &Idl,
    _program_name: &str,
    clients_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = "// \u{1F6D1} DO NOT EDIT - AUTO-GENERATED BY NACLAC\n// Re-run `naclac generate` to refresh this file.\n\n";

    // ── 1. types/typedefs.ts ──────────────────────────────────────────────────
    let mut typedefs_content = header.to_string();
    typedefs_content.push_str("import * as naclac from \"@naclac-fw/client\";\n\n");
    if !idl.defined_types.is_empty() {
        for t in &idl.defined_types {
            if t.name == "Bool" || t.name.starts_with("Opt") {
                continue;
            }
            let type_docs = if t.docs.is_empty() {
                "/** Auto-generated from the program IDL. */\n".to_string()
            } else {
                render_docs_ts(&t.docs, "")
            };
            match &t.ty {
                IdlTypeDefVariants::Enum { variants } => {
                    typedefs_content.push_str(&type_docs);
                    typedefs_content.push_str(&format!("export enum {} {{\n", t.name));
                    for v in variants {
                        typedefs_content.push_str(&render_docs_ts(&v.docs, "  "));
                        typedefs_content.push_str(&format!("  {},\n", v.name));
                    }
                    typedefs_content.push_str("}\n\n");
                }
                IdlTypeDefVariants::Struct { fields } => {
                    typedefs_content.push_str(&type_docs);
                    typedefs_content.push_str(&format!("export interface {} {{\n", t.name));
                    for f in fields {
                        typedefs_content.push_str(&render_docs_ts(&f.docs, "  "));
                        typedefs_content.push_str(&format!(
                            "  {}: {};\n",
                            f.name,
                            map_type_to_ts(&f.ty, idl.is_zero_copy)
                        ));
                    }
                    typedefs_content.push_str("}\n\n");
                }
            }
        }
    }
    if !idl.defined_types.is_empty() {
        fs::write(clients_dir.join("types/typedefs.ts"), typedefs_content)?;
    }

    // ── 2. types/accounts.ts ──────────────────────────────────────────────────
    let mut accounts_content = header.to_string();
    accounts_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
    {
        let mut idents: Vec<String> = Vec::new();
        for acc in &idl.accounts {
            for field in &acc.ty.fields {
                collect_defined_type_names(&field.ty, &mut idents);
            }
        }
        if !idents.is_empty() {
            accounts_content.push_str(&format!(
                "import {{ {} }} from \"./typedefs\";\n",
                idents.join(", ")
            ));
        }
    }
    accounts_content.push('\n');

    for type_def in &idl.accounts {
        // Emit the discriminator constant alongside the interface
        if let Some(disc) = &type_def.discriminator {
            let disc_bytes = disc
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            accounts_content.push_str(&format!(
                "/** 8-byte discriminator prefix for `{}` accounts on-chain. */\nexport const {}_DISCRIMINATOR = new Uint8Array([{}]);\n\n",
                type_def.name,
                type_def.name.to_uppercase().replace(' ', "_"),
                disc_bytes,
            ));
        }

        if type_def.docs.is_empty() {
            accounts_content
                .push_str("/** Auto-generated account interface from the program IDL. */\n");
        } else {
            accounts_content.push_str(&render_docs_ts(&type_def.docs, ""));
        }
        accounts_content.push_str(&format!("export interface {} {{\n", type_def.name));
        for field in &type_def.ty.fields {
            accounts_content.push_str(&render_docs_ts(&field.docs, "  "));
            accounts_content.push_str(&format!(
                "  {}: {};\n",
                field.name,
                map_type_to_ts(&field.ty, idl.is_zero_copy)
            ));
        }
        accounts_content.push_str("}\n\n");
    }
    if !idl.accounts.is_empty() {
        fs::write(clients_dir.join("types/accounts.ts"), accounts_content)?;
    }

    // ── 3. types/events.ts ────────────────────────────────────────────────────
    let mut events_content = header.to_string();
    events_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
    {
        let mut idents: Vec<String> = Vec::new();
        for event_def in &idl.events {
            for field in &event_def.fields {
                collect_defined_type_names(&field.ty, &mut idents);
            }
        }
        if !idents.is_empty() {
            events_content.push_str(&format!(
                "import {{ {} }} from \"./typedefs\";\n",
                idents.join(", ")
            ));
        }
    }
    events_content.push('\n');
    if !idl.events.is_empty() {
        for event_def in &idl.events {
            // Emit discriminator constant for event matching
            if let Some(disc) = &event_def.discriminator {
                let disc_bytes = disc
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                events_content.push_str(&format!(
                    "/** 8-byte discriminator for `{}` events in transaction logs. */\nexport const {}_EVENT_DISCRIMINATOR = new Uint8Array([{}]);\n\n",
                    event_def.name,
                    event_def.name.to_uppercase().replace(' ', "_"),
                    disc_bytes,
                ));
            }

            if event_def.docs.is_empty() {
                events_content
                    .push_str("/** Auto-generated event interface from the program IDL. */\n");
            } else {
                events_content.push_str(&render_docs_ts(&event_def.docs, ""));
            }
            events_content.push_str(&format!("export interface {} {{\n", event_def.name));
            for field in &event_def.fields {
                events_content.push_str(&render_docs_ts(&field.docs, "  "));
                events_content.push_str(&format!(
                    "  {}: {};\n",
                    field.name,
                    map_type_to_ts(&field.ty, idl.is_zero_copy)
                ));
            }
            events_content.push_str("}\n\n");

            events_content.push_str(&format!(
                "/** Subscribes to `{}` events. Returns a listener ID for cleanup. */\nexport function add{}Listener(\n  program: any,\n  callback: (event: {}, slot: number, signature: string) => void\n): number {{\n  return program.addEventListener(\"{}\", callback);\n}}\n\n",
                event_def.name, event_def.name, event_def.name, event_def.name
            ));

            events_content.push_str(&format!(
                "/** Waits for the next `{}` event. Resolves `null` if the timeout expires. */\nexport async function waitFor{}(\n  program: any,\n  options?: {{ timeoutMs?: number }}\n): Promise<{} | null> {{\n  return program.waitForEvent(\"{}\", options) as Promise<{} | null>;\n}}\n\n",
                event_def.name, event_def.name, event_def.name, event_def.name, event_def.name
            ));

            events_content.push_str(&format!(
                "/** Decodes every `{}` event found in an already-fetched list of transaction log lines (e.g. `.rpc()`'s returned `logs`). Race-free, unlike `waitFor{}`/`add{}Listener` — prefer this when you already know which transaction you're checking. */\nexport function parse{}Events(\n  program: any,\n  logs: readonly string[]\n): {}[] {{\n  return program.parseEvents(\"{}\", logs) as {}[];\n}}\n\n",
                event_def.name, event_def.name, event_def.name, event_def.name, event_def.name, event_def.name, event_def.name
            ));
        }
        events_content.push_str("/** Removes a previously registered event listener. */\nexport function removeListener(program: any, listenerId: number) {\n  program.removeEventListener(listenerId);\n}\n");
    }
    if !idl.events.is_empty() {
        fs::write(clients_dir.join("types/events.ts"), events_content)?;
    }

    // ── 4. types/constants.ts ─────────────────────────────────────────────────
    let mut constants_content = header.to_string();
    constants_content.push_str("import * as naclac from \"@naclac-fw/client\";\n\n");
    constants_content.push_str(&format!(
        "/** The on-chain address of this program. */\nexport const PROGRAM_ID = naclac.address(\"{}\");\n",
        idl.address
    ));
    for constant in &idl.constants {
        let ty = map_type_to_ts(&constant.ty, idl.is_zero_copy);
        constants_content.push_str(&render_docs_ts(&constant.docs, ""));
        constants_content.push_str(&format!(
            "export const {}: {} = {};\n",
            constant.name,
            ty,
            format_const_value(&constant.value, constant.ty.as_str().unwrap_or(""))
        ));
    }
    fs::write(clients_dir.join("types/constants.ts"), constants_content)?;

    // ── 5. types/errors.ts ────────────────────────────────────────────────────
    let mut errors_content = header.to_string();
    errors_content.push_str("/** All program errors keyed by their numeric error code. */\nexport const ProgramErrors = {\n");
    for err in &idl.errors {
        // Prefer the modern `message` field; fall back to the legacy `msg` field
        let msg = err
            .message
            .clone()
            .or_else(|| err.msg.clone())
            .unwrap_or_default();
        errors_content.push_str(&format!(
            "  {}: {{ code: {}, name: \"{}\", message: \"{}\" }},\n",
            err.code, err.code, err.name, msg
        ));
    }
    errors_content.push_str("} as const;\n\n");
    errors_content.push_str("export type ProgramErrorCode = keyof typeof ProgramErrors;\n\n");
    errors_content.push_str("/** Returns a human-readable error message for a given program error code. */\nexport function getProgramError(code: number): string | undefined {\n  return (ProgramErrors as any)[code]?.message;\n}\n");
    if !idl.errors.is_empty() {
        fs::write(clients_dir.join("types/errors.ts"), errors_content)?;
    }

    // ── 6. types/index.ts ─────────────────────────────────────────────────────
    let mut types_index = header.to_string();
    if !idl.accounts.is_empty() {
        types_index.push_str("export * from \"./accounts\";\n");
    }
    if !idl.defined_types.is_empty() {
        types_index.push_str("export * from \"./typedefs\";\n");
    }
    if !idl.events.is_empty() {
        types_index.push_str("export * from \"./events\";\n");
    }
    types_index.push_str("export * from \"./constants\";\n");
    if !idl.errors.is_empty() {
        types_index.push_str("export * from \"./errors\";\n");
    }
    fs::write(clients_dir.join("types/index.ts"), types_index)?;

    // ── 7. Generate instructions ───────────────────────────────────────────────
    let mut instructions_index = header.to_string();
    for ix in &idl.instructions {
        // `ix_name` is the raw IDL instruction name — the client library keys
        // `program.methods` by this exact string at runtime, so any call
        // through that lookup must use it unmodified. `ix_camel` is purely
        // the cosmetic export/file/symbol name for the generated wrapper.
        let ix_name = &ix.name;
        let ix_camel = ix_name.to_lower_camel_case();
        let ix_name_pascal = ix_name.to_upper_camel_case();
        instructions_index.push_str(&format!("export * from \"./{ix_camel}\";\n"));

        let mut ix_content = header.to_string();
        // All imports come from @naclac-fw/client — zero direct @solana/* imports
        ix_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
        if ix.args.iter().any(|a| references_defined_type(&a.ty)) {
            ix_content.push_str("import * as types from \"../types/typedefs\";\n");
        }

        // Discriminator constant
        let disc_bytes = ix
            .discriminator
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        ix_content.push_str(&format!(
            "\n/** 8-byte discriminator for the `{ix_name}` instruction. */\nexport const {}_DISCRIMINATOR = new Uint8Array([{disc_bytes}]);\n",
            ix_name.to_uppercase().replace('-', "_")
        ));

        // Args type alias — named for discoverability
        if ix.args.iter().any(|a| a.name != "ctx") {
            ix_content.push_str(&format!("\n/** Instruction arguments for `{ix_camel}`. */\nexport interface {ix_name_pascal}Args {{\n"));
            for arg in &ix.args {
                if arg.name == "ctx" {
                    continue;
                }
                ix_content.push_str(&render_docs_ts(&arg.docs, "  "));
                ix_content.push_str(&format!(
                    "  {}: {};\n",
                    arg.name,
                    map_type_to_ts_with_prefix(&arg.ty, idl.is_zero_copy, "types.")
                ));
            }
            ix_content.push_str("}\n");
        }

        // Accounts type alias — required vs optional clearly typed
        ix_content.push_str(&format!("\n/** Accounts for the `{ix_camel}` instruction. */\nexport interface {ix_name_pascal}Accounts {{\n"));
        for acc in &ix.accounts {
            let name_lower = acc.name.to_lowercase();
            let is_auto = [
                "systemprogram",
                "tokenprogram",
                "token2022program",
                "ataprogram",
                "rent",
                "sysvarrent",
            ]
            .contains(&name_lower.as_str())
                || acc.address.is_some();
            let is_pda = acc.pda.is_some();
            let is_optional = acc.optional.unwrap_or(false);
            let optional_marker = if is_auto || is_pda || is_optional {
                "?"
            } else {
                ""
            };
            ix_content.push_str(&render_docs_ts(&acc.docs, "  "));
            ix_content.push_str(&format!(
                "  {}{}: naclac.Address | string;\n",
                acc.name, optional_marker
            ));
        }
        ix_content.push_str("}\n");

        // The builder function
        let args_param = if ix.args.iter().any(|a| a.name != "ctx") {
            format!("args: {ix_name_pascal}Args")
        } else {
            "args?: Record<string, never>".to_string()
        };
        let mut builder_doc_lines = vec![
            format!("Builds the `{ix_camel}` instruction pipeline."),
            "Call `.rpc()` to send or `.instruction()` to get the raw instruction.".to_string(),
        ];
        if !ix.docs.is_empty() {
            builder_doc_lines.push(String::new());
            builder_doc_lines.extend(ix.docs.iter().cloned());
        }
        ix_content.push('\n');
        ix_content.push_str(&render_docs_ts(&builder_doc_lines, ""));
        ix_content.push_str(&format!(
            "export function {ix_camel}(\n  program: any,\n  {args_param},\n  accounts?: Partial<{ix_name_pascal}Accounts>\n) {{\n"
        ));
        ix_content.push_str(&format!(
            "  const builder = program.methods.{ix_name}(args ?? {{}});\n"
        ));
        ix_content.push_str("  if (accounts) {\n    return builder.accounts(accounts);\n  }\n  return builder;\n}\n");

        fs::write(
            clients_dir.join(format!("instructions/{ix_camel}.ts")),
            ix_content,
        )?;
    }
    if !idl.instructions.is_empty() {
        fs::write(
            clients_dir.join("instructions/index.ts"),
            instructions_index,
        )?;
    }

    Ok(())
}
