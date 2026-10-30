//! The on-chain form of a naclac IDL, and its inverse. Solana Explorer
//! parses an IDL without `metadata.spec` as a legacy Anchor IDL, and finds an
//! account's or event's fields in `types` by name. So the on-chain form is
//! the local IDL with `metadata.spec` added, every account's and event's
//! fields moved out of `accounts`/`events` into `types` beside the defined
//! types, and `definedTypes` renamed `types`. [`to_local_idl`] moves them
//! back, so `fetch` returns exactly the local IDL.

use {
    crate::Idl,
    serde_json::{json, Map, Value},
    std::collections::BTreeSet,
};

/// The `metadata.spec` Explorer routes to its display path for an Anchor
/// 0.30+ IDL.
const ONCHAIN_SPEC: &str = "0.1.0";

fn serialize(value: &impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("Failed to serialize the IDL: {e}"))
}

fn type_entry(name: &str, docs: &[String], ty: Value) -> Value {
    let mut entry = Map::new();
    entry.insert("name".to_string(), Value::from(name));
    if !docs.is_empty() {
        entry.insert("docs".to_string(), json!(docs));
    }
    entry.insert("type".to_string(), ty);
    Value::Object(entry)
}

/// One entry per account and per event (holding its fields), then every
/// defined type. Errors on a name used twice, since Explorer matches by name
/// alone.
fn types_list(idl: &Idl) -> Result<Vec<Value>, String> {
    let mut types: Vec<Value> = Vec::new();
    for account in &idl.accounts {
        types.push(type_entry(
            &account.name,
            &account.docs,
            serialize(&account.ty)?,
        ));
    }
    for event in &idl.events {
        types.push(type_entry(
            &event.name,
            &event.docs,
            json!({ "kind": "struct", "fields": serialize(&event.fields)? }),
        ));
    }
    for defined in &idl.defined_types {
        types.push(serialize(defined)?);
    }

    let mut seen = BTreeSet::new();
    for entry in &types {
        let name = entry["name"].as_str().unwrap_or_default();
        if !seen.insert(name.to_string()) {
            return Err(format!(
                "The name `{name}` is used by more than one account, event or defined type; \
                 Solana Explorer matches fields to them by name, so rename one."
            ));
        }
    }
    Ok(types)
}

/// The IDL to upload on chain. Every key of the local IDL stays where it is,
/// except `accounts` and `events` keep only each entry's name, discriminator
/// and (for an event) `alloc`, and `definedTypes` becomes `types`, holding
/// those accounts' and events' fields too. `metadata.spec` is added.
pub fn to_onchain_idl(idl: &Idl) -> Result<Value, String> {
    let Value::Object(root) = serialize(idl)? else {
        return Err("The IDL did not serialize to a JSON object.".to_string());
    };
    let types = Value::Array(types_list(idl)?);

    let mut onchain = Map::new();
    for (key, value) in root {
        let value = match key.as_str() {
            "metadata" => {
                let mut metadata = value;
                if let Value::Object(map) = &mut metadata {
                    map.insert("spec".to_string(), Value::from(ONCHAIN_SPEC));
                }
                metadata
            }
            "accounts" => Value::Array(
                idl.accounts
                    .iter()
                    .map(|a| json!({ "name": a.name, "discriminator": a.discriminator }))
                    .collect(),
            ),
            "events" => Value::Array(
                idl.events
                    .iter()
                    .map(|e| {
                        let mut entry = Map::new();
                        entry.insert("name".to_string(), Value::from(e.name.as_str()));
                        entry.insert("discriminator".to_string(), json!(e.discriminator));
                        if e.alloc {
                            entry.insert("alloc".to_string(), Value::Bool(true));
                        }
                        Value::Object(entry)
                    })
                    .collect(),
            ),
            "definedTypes" => {
                onchain.insert("types".to_string(), types.clone());
                continue;
            }
            _ => value,
        };
        onchain.insert(key, value);
    }
    Ok(Value::Object(onchain))
}

/// Whether `idl` was written by naclac: only a naclac IDL has
/// `metadata.serialization`.
pub fn is_naclac_idl(idl: &Value) -> bool {
    idl.pointer("/metadata/serialization").is_some()
}

/// Removes and returns the entry of `types` named `name`, leaving the others
/// in order.
fn take_type(types: &mut Vec<Value>, name: &str) -> Option<Value> {
    let index = types
        .iter()
        .position(|entry| entry["name"].as_str() == Some(name))?;
    Some(types.remove(index))
}

fn list<'a>(root: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, String> {
    match root.get(key) {
        Some(Value::Array(items)) => Ok(items),
        _ => Err(format!("The on-chain IDL has no `{key}` list.")),
    }
}

fn entry_name(entry: &Value) -> Result<&str, String> {
    entry["name"]
        .as_str()
        .ok_or_else(|| format!("An entry of the on-chain IDL has no name: {entry}"))
}

/// An account's local form: its on-chain `name` and `discriminator`, plus
/// `docs` and `type` from its entry in `types`.
fn local_account(entry: &Value, types: &mut Vec<Value>) -> Result<Value, String> {
    let name = entry_name(entry)?;
    let ty = take_type(types, name)
        .ok_or_else(|| format!("The account `{name}` has no entry in `types`."))?;
    let mut local = Map::new();
    local.insert("name".to_string(), Value::from(name));
    if let Some(docs) = ty.get("docs") {
        local.insert("docs".to_string(), docs.clone());
    }
    local.insert("discriminator".to_string(), entry["discriminator"].clone());
    local.insert("type".to_string(), ty["type"].clone());
    Ok(Value::Object(local))
}

/// An event's local form: its on-chain `name`, `discriminator` and `alloc`,
/// plus `docs` and `fields` from its entry in `types`.
fn local_event(entry: &Value, types: &mut Vec<Value>) -> Result<Value, String> {
    let name = entry_name(entry)?;
    let ty = take_type(types, name)
        .ok_or_else(|| format!("The event `{name}` has no entry in `types`."))?;
    let mut local = Map::new();
    local.insert("name".to_string(), Value::from(name));
    if let Some(docs) = ty.get("docs") {
        local.insert("docs".to_string(), docs.clone());
    }
    local.insert("discriminator".to_string(), entry["discriminator"].clone());
    if let Some(alloc) = entry.get("alloc") {
        local.insert("alloc".to_string(), alloc.clone());
    }
    local.insert("fields".to_string(), ty["type"]["fields"].clone());
    Ok(Value::Object(local))
}

/// The local IDL a [`to_onchain_idl`] result was made from. Each account and
/// event takes its fields back from the `types` entry of the same name, and
/// the remaining types become `definedTypes`. Only for an IDL for which
/// [`is_naclac_idl`] holds, since another IDL's `types` are not ours to move.
pub fn to_local_idl(onchain: Value) -> Result<Value, String> {
    let Value::Object(root) = onchain else {
        return Err("The on-chain IDL is not a JSON object.".to_string());
    };

    let mut remaining = list(&root, "types")?.clone();
    let accounts = list(&root, "accounts")?
        .iter()
        .map(|entry| local_account(entry, &mut remaining))
        .collect::<Result<Vec<_>, _>>()?;
    let events = list(&root, "events")?
        .iter()
        .map(|entry| local_event(entry, &mut remaining))
        .collect::<Result<Vec<_>, _>>()?;

    let mut local = Map::new();
    for (key, value) in root {
        let value = match key.as_str() {
            "metadata" => {
                let mut metadata = value;
                if let Value::Object(map) = &mut metadata {
                    map.remove("spec");
                }
                metadata
            }
            "accounts" => Value::Array(accounts.clone()),
            "events" => Value::Array(events.clone()),
            "types" => {
                local.insert("definedTypes".to_string(), Value::Array(remaining.clone()));
                continue;
            }
            _ => value,
        };
        local.insert(key, value);
    }
    Ok(Value::Object(local))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::to_compact_pretty_json};

    fn sample_idl() -> Idl {
        let json = r#"{
            "address": "A",
            "metadata": {"name": "n", "version": "0.1.0", "description": "d", "serialization": "wincode"},
            "instructions": [{
                "name": "ix",
                "discriminator": [1],
                "accounts": [{"name": "maybe", "optional": true}, {"name": "always"}],
                "args": [{"name": "items", "type": {"vec": "u8"}, "slots": 4}]
            }],
            "accounts": [{"name": "Acc", "docs": ["an account"], "discriminator": [2], "type": {"kind": "struct", "fields": [{"name": "x", "type": "publicKey"}]}}],
            "events": [
                {"name": "Ev", "discriminator": [3], "fields": [{"name": "y", "type": "u64"}]},
                {"name": "EvAlloc", "docs": ["packed"], "discriminator": [4], "alloc": true, "fields": [{"name": "w", "type": "u8"}]}
            ],
            "errors": [{"code": 6000, "name": "Bad", "msg": "it is bad"}, {"code": 6001, "name": "Silent"}],
            "constants": [{"name": "C", "type": "bytes", "value": "[1, 2]"}],
            "definedTypes": [
                {"name": "Plain", "type": {"kind": "struct", "fields": [{"name": "z", "type": "u8"}]}},
                {"name": "Kind", "type": {"kind": "enum", "variants": [{"name": "A", "discriminant": "0"}]}}
            ],
            "pdas": [],
            "eventCpiTag": [9, 9]
        }"#;
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_spec_is_added_to_the_metadata() {
        let onchain = to_onchain_idl(&sample_idl()).unwrap();
        assert_eq!(onchain["metadata"]["spec"], Value::from("0.1.0"));
        assert_eq!(onchain["metadata"]["serialization"], Value::from("wincode"));
    }

    #[test]
    fn accounts_and_events_keep_only_their_name_and_discriminator() {
        let onchain = to_onchain_idl(&sample_idl()).unwrap();
        assert_eq!(
            onchain["accounts"],
            json!([{ "name": "Acc", "discriminator": [2] }])
        );
        assert_eq!(
            onchain["events"],
            json!([
                { "name": "Ev", "discriminator": [3] },
                { "name": "EvAlloc", "discriminator": [4], "alloc": true }
            ])
        );
    }

    #[test]
    fn types_replace_defined_types_and_hold_accounts_then_events_then_defined() {
        let onchain = to_onchain_idl(&sample_idl()).unwrap();
        assert!(onchain.get("definedTypes").is_none());
        let types = onchain["types"].as_array().unwrap();
        let names: Vec<&str> = types.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Acc", "Ev", "EvAlloc", "Plain", "Kind"]);
        assert_eq!(types[0]["type"]["fields"][0]["name"], Value::from("x"));
        assert_eq!(types[0]["docs"], json!(["an account"]));
        assert_eq!(types[1]["type"]["kind"], Value::from("struct"));
        assert_eq!(types[1]["type"]["fields"][0]["name"], Value::from("y"));
        assert_eq!(types[4]["type"]["kind"], Value::from("enum"));
    }

    #[test]
    fn everything_else_in_the_local_idl_is_unchanged() {
        let idl = sample_idl();
        let onchain = to_onchain_idl(&idl).unwrap();
        let local = serde_json::to_value(&idl).unwrap();
        for key in [
            "address",
            "instructions",
            "errors",
            "constants",
            "pdas",
            "eventCpiTag",
        ] {
            assert_eq!(onchain[key], local[key], "{key}");
        }
        assert_eq!(onchain["errors"][0]["msg"], Value::from("it is bad"));
        assert!(onchain["errors"][1].get("msg").is_none());
        assert_eq!(
            onchain["instructions"][0]["args"][0]["slots"],
            Value::from(4)
        );
    }

    #[test]
    fn moving_the_fields_back_gives_the_local_idl_exactly() {
        let idl = sample_idl();
        let onchain = to_onchain_idl(&idl).unwrap();
        assert!(is_naclac_idl(&onchain));
        let restored = to_local_idl(onchain).unwrap();
        assert_eq!(
            to_compact_pretty_json(&restored).unwrap(),
            to_compact_pretty_json(&idl).unwrap()
        );
    }

    #[test]
    fn an_account_without_a_types_entry_cannot_be_restored() {
        let mut onchain = to_onchain_idl(&sample_idl()).unwrap();
        if let Value::Array(types) = &mut onchain["types"] {
            types.retain(|t| t["name"] != "Acc");
        }
        let error = to_local_idl(onchain).unwrap_err();
        assert!(error.contains("`Acc`"), "{error}");
    }

    #[test]
    fn an_idl_without_metadata_serialization_is_not_a_naclac_idl() {
        let anchor: Value = serde_json::from_str(
            r#"{"address": "A", "metadata": {"name": "n", "version": "0.1.0", "spec": "0.1.0"}}"#,
        )
        .unwrap();
        assert!(!is_naclac_idl(&anchor));
    }

    #[test]
    fn a_name_shared_by_an_account_and_a_defined_type_is_an_error() {
        let mut idl = sample_idl();
        idl.defined_types[0].name = "Acc".to_string();
        let error = to_onchain_idl(&idl).unwrap_err();
        assert!(error.contains("`Acc`"), "{error}");
    }
}
