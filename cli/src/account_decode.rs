//! Decodes the bytes of one account of a Naclac program into JSON, following
//! the program's IDL and its `metadata.serialization`. The layouts mirror the
//! TypeScript client's account coder (`client/src/coder/types.ts`): a
//! `bytemuck` account is packed (fields in order, no padding, enums padded to
//! their largest variant), a `borsh` or `wincode` account is sequential with
//! `u32` lengths and one-byte tags. A type an account of that serialization
//! cannot store is an error that names the field, never a guess.

use {
    naclac_idl::{
        Idl, IdlAccountStruct, IdlEnumFields, IdlEnumVariant, IdlField, IdlTypeDef, Serialization,
    },
    serde_json::{json, Map, Number, Value},
};

const FIXED_LEN_WIDTH: usize = 2;
const MAX_DEPTH: usize = 32;

/// An account's fields as JSON, and how many bytes follow the last field.
pub struct Decoded {
    pub value: Value,
    pub trailing: usize,
}

/// Decodes `data`, which starts with the account's discriminator, as the
/// account `account` of `idl`.
pub fn decode_account(
    idl: &Idl,
    account: &IdlAccountStruct,
    data: &[u8],
) -> Result<Decoded, String> {
    let decoder = Decoder {
        idl,
        packed: idl.metadata.serialization == Serialization::Bytemuck,
        data,
    };
    let mut at = account.discriminator.len();
    if data.len() < at {
        return Err(format!(
            "the account data is {} bytes, shorter than its {at}-byte discriminator",
            data.len()
        ));
    }
    let value = decoder.read_fields(&account.name, &account.ty.fields, &mut at, 0)?;
    Ok(Decoded {
        value: Value::Object(value),
        trailing: data.len() - at,
    })
}

struct Decoder<'a> {
    idl: &'a Idl,
    packed: bool,
    data: &'a [u8],
}

impl<'a> Decoder<'a> {
    fn take(&self, at: &mut usize, count: usize, path: &str) -> Result<&'a [u8], String> {
        let end = at
            .checked_add(count)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| {
                format!(
                    "the account data is {} bytes, but `{path}` needs {count} more at byte {at}",
                    self.data.len()
                )
            })?;
        let bytes = &self.data[*at..end];
        *at = end;
        Ok(bytes)
    }

    fn field_type(&self, owner: &str, field: &IdlField) -> Result<Value, String> {
        match field.slots {
            None => Ok(field.ty.clone()),
            Some(slots) => {
                if !self.packed {
                    return Err(format!(
                        "`{owner}.{}` has `slots`, but the IDL's serialization is not bytemuck",
                        field.name
                    ));
                }
                fixed_form(&field.ty, slots).map_err(|e| format!("`{owner}.{}`: {e}", field.name))
            }
        }
    }

    fn read_fields(
        &self,
        owner: &str,
        fields: &[IdlField],
        at: &mut usize,
        depth: usize,
    ) -> Result<Map<String, Value>, String> {
        let mut out = Map::new();
        for field in fields {
            let ty = self.field_type(owner, field)?;
            let path = format!("{owner}.{}", field.name);
            out.insert(field.name.clone(), self.read(&ty, at, &path, depth)?);
        }
        Ok(out)
    }

    fn read(&self, ty: &Value, at: &mut usize, path: &str, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!("`{path}` nests types more than {MAX_DEPTH} deep"));
        }
        match ty {
            Value::String(name) => self.read_named(name, at, path),
            Value::Object(shape) => {
                if let Some(array) = shape.get("array") {
                    let (inner, length) = array_parts(array, path)?;
                    let mut items = Vec::new();
                    for index in 0..length {
                        items.push(self.read(inner, at, &format!("{path}[{index}]"), depth + 1)?);
                    }
                    Ok(Value::Array(items))
                } else if let Some(fixed) = shape.get("fixedVec") {
                    let (inner, capacity) = array_parts(fixed, path)?;
                    let start = *at;
                    let length = self.fixed_length(at, path)?.min(capacity);
                    let item_size = self.size(inner, path, depth + 1)?;
                    let mut items = Vec::new();
                    for index in 0..length {
                        let mut item_at = start + FIXED_LEN_WIDTH + index * item_size;
                        items.push(self.read(
                            inner,
                            &mut item_at,
                            &format!("{path}[{index}]"),
                            depth + 1,
                        )?);
                    }
                    *at = start + FIXED_LEN_WIDTH + capacity * item_size;
                    if *at > self.data.len() {
                        return Err(self.too_short(path, *at));
                    }
                    Ok(Value::Array(items))
                } else if let Some(capacity) = shape.get("fixedString") {
                    let capacity = capacity
                        .as_u64()
                        .and_then(|c| usize::try_from(c).ok())
                        .ok_or_else(|| format!("`{path}`: fixedString capacity is not a number"))?;
                    let length = self.fixed_length(at, path)?.min(capacity);
                    let bytes = self.take(at, capacity, path)?;
                    utf8(&bytes[..length], path).map(Value::String)
                } else if let Some(inner) = shape.get("vec") {
                    self.require_borsh("vec", path)?;
                    let count = self.length_prefix(at, path)?;
                    if count > self.data.len() - (*at).min(self.data.len()) {
                        return Err(format!(
                            "`{path}` claims {count} items, more than the bytes that remain"
                        ));
                    }
                    let mut items = Vec::new();
                    for index in 0..count {
                        items.push(self.read(inner, at, &format!("{path}[{index}]"), depth + 1)?);
                    }
                    Ok(Value::Array(items))
                } else if let Some(inner) = shape.get("option") {
                    self.require_borsh("option", path)?;
                    match self.take(at, 1, path)?[0] {
                        0 => Ok(Value::Null),
                        1 => match self.read(inner, at, path, depth + 1)? {
                            Value::Null => Err(format!(
                                "`{path}` is an Option holding an Option, which JSON cannot tell \
                                 from an empty one"
                            )),
                            value => Ok(value),
                        },
                        tag => Err(format!(
                            "`{path}` has the Option tag {tag}; expected 0 or 1"
                        )),
                    }
                } else if let Some(Value::String(name)) = shape.get("defined") {
                    self.read_defined(name, at, path, depth)
                } else {
                    Err(format!("`{path}` has the unsupported type {ty}"))
                }
            }
            _ => Err(format!("`{path}` has the unsupported type {ty}")),
        }
    }

    fn read_named(&self, name: &str, at: &mut usize, path: &str) -> Result<Value, String> {
        Ok(match name {
            "u8" => unsigned(self.take(at, 1, path)?),
            "u16" => unsigned(self.take(at, 2, path)?),
            "u32" => unsigned(self.take(at, 4, path)?),
            "u64" => unsigned(self.take(at, 8, path)?),
            "u128" => unsigned(self.take(at, 16, path)?),
            "i8" => signed(self.take(at, 1, path)?),
            "i16" => signed(self.take(at, 2, path)?),
            "i32" => signed(self.take(at, 4, path)?),
            "i64" => signed(self.take(at, 8, path)?),
            "i128" => signed(self.take(at, 16, path)?),
            "f32" => {
                let bytes = self.take(at, 4, path)?;
                float(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string())
            }
            "f64" => {
                let bytes = self.take(at, 8, path)?;
                let mut raw = [0u8; 8];
                raw.copy_from_slice(bytes);
                float(f64::from_le_bytes(raw).to_string())
            }
            "bool" => {
                let byte = self.take(at, 1, path)?[0];
                if self.packed {
                    Value::Bool(byte != 0)
                } else {
                    match byte {
                        0 => Value::Bool(false),
                        1 => Value::Bool(true),
                        other => {
                            return Err(format!(
                                "`{path}` is a bool stored as {other}; expected 0 or 1"
                            ))
                        }
                    }
                }
            }
            "publicKey" => Value::String(bs58::encode(self.take(at, 32, path)?).into_string()),
            "string" | "String" => {
                self.require_borsh("string", path)?;
                let length = self.length_prefix(at, path)?;
                Value::String(utf8(self.take(at, length, path)?, path)?)
            }
            other => return Err(self.unsupported(other, path)),
        })
    }

    fn read_defined(
        &self,
        name: &str,
        at: &mut usize,
        path: &str,
        depth: usize,
    ) -> Result<Value, String> {
        match &self.defined(name, path)? {
            IdlTypeDef::Struct { fields, .. } => self
                .read_fields(name, fields, at, depth + 1)
                .map(Value::Object),
            IdlTypeDef::Enum { variants, repr, .. } => {
                let start = *at;
                let value = if self.packed {
                    let tag_type = repr.as_deref().unwrap_or("u8");
                    let tag = self.read_named(tag_type, at, path)?;
                    let variant = find_variant(variants, &tag_text(&tag), name, path)?;
                    let value = self.read_variant(variant, at, path, depth)?;
                    *at = start + self.enum_size(variants, tag_type, path, depth)?;
                    if *at > self.data.len() {
                        return Err(self.too_short(path, *at));
                    }
                    value
                } else {
                    let tag = self.take(at, 1, path)?[0];
                    let variant = find_variant(variants, &tag.to_string(), name, path)?;
                    self.read_variant(variant, at, path, depth)?
                };
                Ok(value)
            }
        }
    }

    fn read_variant(
        &self,
        variant: &IdlEnumVariant,
        at: &mut usize,
        path: &str,
        depth: usize,
    ) -> Result<Value, String> {
        let mut out = Map::new();
        out.insert("kind".to_string(), Value::String(variant.name.clone()));
        let variant_path = format!("{path}::{}", variant.name);
        match &variant.fields {
            None => {}
            Some(IdlEnumFields::Named(fields)) => {
                out.extend(self.read_fields(&variant_path, fields, at, depth + 1)?);
            }
            Some(IdlEnumFields::Tuple(types)) => {
                let mut items = Vec::new();
                for (index, ty) in types.iter().enumerate() {
                    items.push(self.read(ty, at, &format!("{variant_path}.{index}"), depth + 1)?);
                }
                out.insert("fields".to_string(), Value::Array(items));
            }
        }
        Ok(Value::Object(out))
    }

    fn size(&self, ty: &Value, path: &str, depth: usize) -> Result<usize, String> {
        if depth > MAX_DEPTH {
            return Err(format!("`{path}` nests types more than {MAX_DEPTH} deep"));
        }
        match ty {
            Value::String(name) => match name.as_str() {
                "u8" | "i8" | "bool" => Ok(1),
                "u16" | "i16" => Ok(2),
                "u32" | "i32" | "f32" => Ok(4),
                "u64" | "i64" | "f64" => Ok(8),
                "u128" | "i128" => Ok(16),
                "publicKey" => Ok(32),
                other => Err(self.unsupported(other, path)),
            },
            Value::Object(shape) => {
                if let Some(array) = shape.get("array") {
                    let (inner, length) = array_parts(array, path)?;
                    self.size(inner, path, depth + 1)?
                        .checked_mul(length)
                        .ok_or_else(|| format!("`{path}` is too large"))
                } else if let Some(fixed) = shape.get("fixedVec") {
                    let (inner, capacity) = array_parts(fixed, path)?;
                    self.size(inner, path, depth + 1)?
                        .checked_mul(capacity)
                        .and_then(|slots| slots.checked_add(FIXED_LEN_WIDTH))
                        .ok_or_else(|| format!("`{path}` is too large"))
                } else if let Some(capacity) = shape.get("fixedString") {
                    capacity
                        .as_u64()
                        .and_then(|c| usize::try_from(c).ok())
                        .and_then(|c| c.checked_add(FIXED_LEN_WIDTH))
                        .ok_or_else(|| format!("`{path}`: fixedString capacity is not a number"))
                } else if let Some(Value::String(name)) = shape.get("defined") {
                    match &self.defined(name, path)? {
                        IdlTypeDef::Struct { fields, .. } => {
                            self.fields_size(name, fields, path, depth)
                        }
                        IdlTypeDef::Enum { variants, repr, .. } => {
                            self.enum_size(variants, repr.as_deref().unwrap_or("u8"), path, depth)
                        }
                    }
                } else {
                    Err(self.unsupported(&ty.to_string(), path))
                }
            }
            _ => Err(self.unsupported(&ty.to_string(), path)),
        }
    }

    fn fields_size(
        &self,
        owner: &str,
        fields: &[IdlField],
        path: &str,
        depth: usize,
    ) -> Result<usize, String> {
        let mut total = 0usize;
        for field in fields {
            let ty = self.field_type(owner, field)?;
            total = total
                .checked_add(self.size(&ty, &format!("{owner}.{}", field.name), depth + 1)?)
                .ok_or_else(|| format!("`{path}` is too large"))?;
        }
        Ok(total)
    }

    fn enum_size(
        &self,
        variants: &[IdlEnumVariant],
        tag_type: &str,
        path: &str,
        depth: usize,
    ) -> Result<usize, String> {
        let tag_size = self.size(&Value::String(tag_type.to_string()), path, depth + 1)?;
        let mut largest = tag_size;
        for variant in variants {
            let variant_path = format!("{path}::{}", variant.name);
            let payload = match &variant.fields {
                None => 0,
                Some(IdlEnumFields::Named(fields)) => {
                    self.fields_size(&variant_path, fields, path, depth)?
                }
                Some(IdlEnumFields::Tuple(types)) => {
                    let mut total = 0usize;
                    for (index, ty) in types.iter().enumerate() {
                        total += self.size(ty, &format!("{variant_path}.{index}"), depth + 1)?;
                    }
                    total
                }
            };
            largest = largest.max(tag_size + payload);
        }
        Ok(largest)
    }

    fn defined(&self, name: &str, path: &str) -> Result<IdlTypeDef, String> {
        self.idl
            .defined_types
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.ty.clone())
            .ok_or_else(|| {
                format!("`{path}` uses the type `{name}`, which the IDL does not define")
            })
    }

    fn fixed_length(&self, at: &mut usize, path: &str) -> Result<usize, String> {
        let bytes = self.take(at, FIXED_LEN_WIDTH, path)?;
        Ok(usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
    }

    fn length_prefix(&self, at: &mut usize, path: &str) -> Result<usize, String> {
        let bytes = self.take(at, 4, path)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    fn require_borsh(&self, what: &str, path: &str) -> Result<(), String> {
        if self.packed {
            return Err(format!(
                "`{path}` is a {what}, which a bytemuck account cannot store (use `slots` for a \
                 fixed-capacity vec or string)"
            ));
        }
        Ok(())
    }

    fn too_short(&self, path: &str, needed: usize) -> String {
        format!(
            "the account data is {} bytes, but `{path}` extends to byte {needed}",
            self.data.len()
        )
    }

    fn unsupported(&self, ty: &str, path: &str) -> String {
        format!("`{path}` has the type {ty}, which `naclac account` cannot decode")
    }
}

fn fixed_form(ty: &Value, slots: u64) -> Result<Value, String> {
    match ty {
        Value::String(name) if name == "string" => Ok(json!({ "fixedString": slots })),
        Value::Object(shape) => {
            if let Some(inner) = shape.get("vec") {
                Ok(json!({ "fixedVec": [inner, slots] }))
            } else if let Some(Value::Array(parts)) = shape.get("array") {
                match parts.as_slice() {
                    [inner, length] => Ok(json!({ "array": [fixed_form(inner, slots)?, length] })),
                    _ => Err("an array type without its length".to_string()),
                }
            } else {
                Err(format!(
                    "`slots` is set on {ty}, which is not a vec or string"
                ))
            }
        }
        _ => Err(format!(
            "`slots` is set on {ty}, which is not a vec or string"
        )),
    }
}

fn array_parts<'v>(parts: &'v Value, path: &str) -> Result<(&'v Value, usize), String> {
    match parts.as_array().map(Vec::as_slice) {
        Some([inner, length]) => length
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .map(|n| (inner, n))
            .ok_or_else(|| format!("`{path}`: the length of the array is not a number")),
        _ => Err(format!("`{path}`: expected [type, length]")),
    }
}

fn find_variant<'v>(
    variants: &'v [IdlEnumVariant],
    discriminant: &str,
    enum_name: &str,
    path: &str,
) -> Result<&'v IdlEnumVariant, String> {
    variants
        .iter()
        .find(|v| v.discriminant == discriminant)
        .ok_or_else(|| {
            let known: Vec<&str> = variants.iter().map(|v| v.discriminant.as_str()).collect();
            format!(
                "`{path}` holds the {enum_name} tag {discriminant}, which no variant declares \
                 (expected one of {}); a tag of 0 usually means the field was never set",
                known.join(", ")
            )
        })
}

fn tag_text(tag: &Value) -> String {
    match tag {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn utf8(bytes: &[u8], path: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|e| format!("`{path}` is not valid UTF-8: {e}"))
}

fn unsigned(bytes: &[u8]) -> Value {
    let mut raw = [0u8; 16];
    raw[..bytes.len()].copy_from_slice(bytes);
    let value = u128::from_le_bytes(raw);
    match u64::try_from(value) {
        Ok(small) if bytes.len() < 16 => Value::Number(Number::from(small)),
        _ => Value::String(value.to_string()),
    }
}

fn signed(bytes: &[u8]) -> Value {
    let mut raw = [0u8; 16];
    raw[..bytes.len()].copy_from_slice(bytes);
    let shift = 128 - 8 * bytes.len() as u32;
    let value = ((u128::from_le_bytes(raw) << shift) as i128) >> shift;
    match i64::try_from(value) {
        Ok(small) if bytes.len() < 16 => Value::Number(Number::from(small)),
        _ => Value::String(value.to_string()),
    }
}

fn float(text: String) -> Value {
    text.parse::<f64>()
        .ok()
        .and_then(Number::from_f64)
        .map_or(Value::String(text), Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idl(serialization: &str, accounts: Value, types: Value) -> Idl {
        serde_json::from_value(json!({
            "address": "11111111111111111111111111111111",
            "metadata": {
                "name": "t", "version": "0.1.0", "description": "", "serialization": serialization
            },
            "instructions": [],
            "accounts": accounts,
            "events": [],
            "errors": [],
            "constants": [],
            "definedTypes": types,
            "pdas": []
        }))
        .unwrap()
    }

    fn account(fields: Value) -> Value {
        json!([{
            "name": "Acc",
            "discriminator": [1, 2],
            "type": { "kind": "struct", "fields": fields }
        }])
    }

    fn decode(idl: &Idl, data: &[u8]) -> Result<Decoded, String> {
        decode_account(idl, &idl.accounts[0], data)
    }

    #[test]
    fn a_packed_account_reads_its_fields_after_the_discriminator() {
        let idl = idl(
            "bytemuck",
            account(json!([
                { "name": "count", "type": "u64" },
                { "name": "authority", "type": "publicKey" }
            ])),
            json!([]),
        );
        let mut data = vec![1, 2];
        data.extend(7u64.to_le_bytes());
        data.extend([0u8; 32]);
        let decoded = decode(&idl, &data).unwrap();
        assert_eq!(decoded.value["count"], json!(7));
        assert_eq!(
            decoded.value["authority"],
            json!("11111111111111111111111111111111")
        );
        assert_eq!(decoded.trailing, 0);
    }

    #[test]
    fn signed_integers_keep_their_sign_and_wide_ones_are_text() {
        let idl = idl(
            "bytemuck",
            account(json!([
                { "name": "a", "type": "i8" },
                { "name": "b", "type": "i16" },
                { "name": "c", "type": "i64" },
                { "name": "d", "type": "i128" },
                { "name": "e", "type": "u128" }
            ])),
            json!([]),
        );
        let mut data = vec![1, 2, 0xff];
        data.extend((-2i16).to_le_bytes());
        data.extend(i64::MIN.to_le_bytes());
        data.extend((-3i128).to_le_bytes());
        data.extend(u128::MAX.to_le_bytes());
        let decoded = decode(&idl, &data).unwrap().value;
        assert_eq!(decoded["a"], json!(-1));
        assert_eq!(decoded["b"], json!(-2));
        assert_eq!(decoded["c"], json!(i64::MIN));
        assert_eq!(decoded["d"], json!("-3"));
        assert_eq!(decoded["e"], json!(u128::MAX.to_string()));
    }

    #[test]
    fn data_that_ends_inside_a_field_is_an_error_naming_it() {
        let idl = idl(
            "bytemuck",
            account(json!([{ "name": "count", "type": "u64" }])),
            json!([]),
        );
        let error = decode(&idl, &[1, 2, 0, 0]).err().unwrap();
        assert!(error.contains("Acc.count"), "{error}");
    }

    #[test]
    fn bytes_after_the_last_field_are_counted() {
        let idl = idl(
            "bytemuck",
            account(json!([{ "name": "flag", "type": "bool" }])),
            json!([]),
        );
        assert_eq!(decode(&idl, &[1, 2, 1, 9, 9]).unwrap().trailing, 2);
    }

    #[test]
    fn a_borsh_account_reads_strings_options_and_vecs() {
        let idl = idl(
            "borsh",
            account(json!([
                { "name": "name", "type": "string" },
                { "name": "maybe", "type": { "option": "u8" } },
                { "name": "none", "type": { "option": "u8" } },
                { "name": "list", "type": { "vec": "u16" } }
            ])),
            json!([]),
        );
        let mut data = vec![1, 2];
        data.extend(2u32.to_le_bytes());
        data.extend(b"hi");
        data.extend([1, 5, 0]);
        data.extend(2u32.to_le_bytes());
        data.extend(1u16.to_le_bytes());
        data.extend(2u16.to_le_bytes());
        let decoded = decode(&idl, &data).unwrap().value;
        assert_eq!(decoded["name"], json!("hi"));
        assert_eq!(decoded["maybe"], json!(5));
        assert_eq!(decoded["none"], Value::Null);
        assert_eq!(decoded["list"], json!([1, 2]));
    }

    #[test]
    fn a_bool_other_than_zero_or_one_is_an_error_in_borsh_but_true_when_packed() {
        let fields = account(json!([{ "name": "flag", "type": "bool" }]));
        let borsh = idl("borsh", fields.clone(), json!([]));
        assert!(decode(&borsh, &[1, 2, 7]).is_err());
        let packed = idl("bytemuck", fields, json!([]));
        assert_eq!(
            decode(&packed, &[1, 2, 7]).unwrap().value["flag"],
            json!(true)
        );
    }

    #[test]
    fn a_borsh_enum_is_found_by_its_discriminant() {
        let idl = idl(
            "wincode",
            account(json!([{ "name": "state", "type": { "defined": "State" } }])),
            json!([{
                "name": "State",
                "type": { "kind": "enum", "variants": [
                    { "name": "Idle", "discriminant": "0" },
                    { "name": "Busy", "discriminant": "4",
                      "fields": [{ "name": "since", "type": "u32" }] }
                ]}
            }]),
        );
        let mut data = vec![1, 2, 4];
        data.extend(9u32.to_le_bytes());
        assert_eq!(
            decode(&idl, &data).unwrap().value["state"],
            json!({ "kind": "Busy", "since": 9 })
        );
        assert!(decode(&idl, &[1, 2, 3]).err().unwrap().contains("tag 3"));
    }

    #[test]
    fn a_packed_enum_takes_the_size_of_its_largest_variant() {
        let idl = idl(
            "bytemuck",
            account(json!([
                { "name": "state", "type": { "defined": "State" } },
                { "name": "after", "type": "u8" }
            ])),
            json!([{
                "name": "State",
                "type": { "kind": "enum", "variants": [
                    { "name": "Idle", "discriminant": "0" },
                    { "name": "Busy", "discriminant": "1", "fields": [{ "name": "since", "type": "u32" }] }
                ]}
            }]),
        );
        let data = vec![1, 2, 0, 0, 0, 0, 0, 42];
        let decoded = decode(&idl, &data).unwrap().value;
        assert_eq!(decoded["state"], json!({ "kind": "Idle" }));
        assert_eq!(decoded["after"], json!(42));
    }

    #[test]
    fn a_slots_field_is_a_fixed_capacity_vec_or_string() {
        let idl = idl(
            "bytemuck",
            account(json!([
                { "name": "ids", "type": { "vec": "u32" }, "slots": 3 },
                { "name": "label", "type": "string", "slots": 4 },
                { "name": "after", "type": "u8" }
            ])),
            json!([]),
        );
        let mut data = vec![1, 2];
        data.extend(2u16.to_le_bytes());
        data.extend(10u32.to_le_bytes());
        data.extend(20u32.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(2u16.to_le_bytes());
        data.extend(b"ab\0\0");
        data.push(5);
        let decoded = decode(&idl, &data).unwrap().value;
        assert_eq!(decoded["ids"], json!([10, 20]));
        assert_eq!(decoded["label"], json!("ab"));
        assert_eq!(decoded["after"], json!(5));
    }

    #[test]
    fn slots_on_a_borsh_account_is_an_error() {
        let idl = idl(
            "borsh",
            account(json!([{ "name": "ids", "type": { "vec": "u32" }, "slots": 3 }])),
            json!([]),
        );
        assert!(decode(&idl, &[1, 2]).err().unwrap().contains("slots"));
    }

    #[test]
    fn a_vec_in_a_bytemuck_account_is_an_error() {
        let idl = idl(
            "bytemuck",
            account(json!([{ "name": "ids", "type": { "vec": "u32" } }])),
            json!([]),
        );
        assert!(decode(&idl, &[1, 2, 0, 0, 0, 0])
            .err()
            .unwrap()
            .contains("bytemuck"));
    }

    #[test]
    fn a_nested_struct_and_an_array_are_read_in_order() {
        let idl = idl(
            "bytemuck",
            account(json!([
                { "name": "pair", "type": { "defined": "Pair" } },
                { "name": "grid", "type": { "array": ["u8", 3] } }
            ])),
            json!([{
                "name": "Pair",
                "type": { "kind": "struct", "fields": [
                    { "name": "x", "type": "u16" }, { "name": "y", "type": "u16" }
                ]}
            }]),
        );
        let decoded = decode(&idl, &[1, 2, 1, 0, 2, 0, 7, 8, 9]).unwrap().value;
        assert_eq!(decoded["pair"], json!({ "x": 1, "y": 2 }));
        assert_eq!(decoded["grid"], json!([7, 8, 9]));
    }

    #[test]
    fn a_type_the_idl_does_not_define_is_an_error() {
        let idl = idl(
            "bytemuck",
            account(json!([{ "name": "pair", "type": { "defined": "Missing" } }])),
            json!([]),
        );
        assert!(decode(&idl, &[1, 2]).err().unwrap().contains("Missing"));
    }

    #[test]
    fn a_float_that_is_not_finite_is_shown_as_text() {
        let idl = idl(
            "bytemuck",
            account(json!([{ "name": "x", "type": "f64" }])),
            json!([]),
        );
        let mut data = vec![1, 2];
        data.extend(f64::NAN.to_le_bytes());
        assert_eq!(decode(&idl, &data).unwrap().value["x"], json!("NaN"));
        let mut data = vec![1, 2];
        data.extend(1.5f64.to_le_bytes());
        assert_eq!(decode(&idl, &data).unwrap().value["x"], json!(1.5));
    }
}
