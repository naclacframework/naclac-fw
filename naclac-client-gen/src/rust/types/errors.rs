use crate::Idl;
use heck::ToUpperCamelCase;
use std::fs;
use std::path::Path;

pub fn generate(
    idl: &Idl,
    clients_dir: &Path,
    header: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if idl.errors.is_empty() {
        return Ok(());
    }

    let mut errors_content = header.to_string();
    errors_content
        .push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum ProgramError {\n");
    for err in &idl.errors {
        let err_camel = err.name.to_upper_camel_case();
        errors_content.push_str(&format!("    {},\n", err_camel));
    }
    errors_content.push_str("}\n\n");

    errors_content.push_str("impl ProgramError {\n");
    errors_content
        .push_str("    pub fn from_code(code: u32) -> Option<Self> {\n        match code {\n");
    for err in &idl.errors {
        let err_camel = err.name.to_upper_camel_case();
        errors_content.push_str(&format!(
            "            {} => Some(Self::{}),\n",
            err.code, err_camel
        ));
    }
    errors_content.push_str("            _ => None,\n        }\n    }\n\n");

    errors_content.push_str("    pub fn message(&self) -> &'static str {\n        match self {\n");
    for err in &idl.errors {
        let err_camel = err.name.to_upper_camel_case();
        // Prefer the modern `message` field; fall back to legacy `msg`
        let msg = err.message.as_deref().or(err.msg.as_deref()).unwrap_or("");
        errors_content.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            err_camel, msg
        ));
    }
    errors_content.push_str("        }\n    }\n}\n\n");
    fs::write(clients_dir.join("src/types/errors.rs"), errors_content).unwrap();

    Ok(())
}
