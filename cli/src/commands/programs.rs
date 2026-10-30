//! Which programs of a workspace a command works on. Local commands (build,
//! check, test, ...) work on the programs listed in any `[programs.<cluster>]`
//! table of Naclac.toml, narrowed by `-p`; commands that talk to a cluster use
//! `workspace::on_cluster`. A program is never picked by scanning folders, and
//! a missing piece of configuration is an error, never a default.

use {
    crate::{commands::keypair, ui},
    std::path::{Path, PathBuf},
    toml::Value,
};

/// Every program listed in any `[programs.<cluster>]` table, each once, in
/// the order first listed.
pub fn listed(config: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let tables = config.get("programs").and_then(Value::as_table);
    for table in tables.into_iter().flat_map(|t| t.values()) {
        for name in table.as_table().into_iter().flat_map(|t| t.keys()) {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    if names.is_empty() {
        ui::error(
            "No programs are listed in Naclac.toml. List each program under a \
             `[programs.<cluster>]` table, e.g. `[programs.localnet]`.",
        );
    }
    names
}

/// `requested` (the names given with `-p`), each of which must be in
/// `available`; all of `available` when none was requested.
pub fn select(available: &[String], requested: &[String]) -> Vec<String> {
    if requested.is_empty() {
        return available.to_vec();
    }
    for name in requested {
        if !available.contains(name) {
            ui::error(format!(
                "`{name}` is not a program this command can use. Available: {}.",
                available.join(", ")
            ));
        }
    }
    requested.to_vec()
}

fn has_lib(program_dir: &Path) -> bool {
    let lib = program_dir.join("src/lib.rs");
    lib.try_exists()
        .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", lib.display())))
}

/// The folder of a listed program, which must hold `src/lib.rs`.
pub fn program_dir(workspace_root: &Path, name: &str) -> PathBuf {
    let dir = workspace_root.join("programs").join(name);
    if !has_lib(&dir) {
        ui::error(format!(
            "`{name}` is listed in Naclac.toml but `programs/{name}/src/lib.rs` does not exist."
        ));
    }
    dir
}

/// Every program folder of the workspace, listed or not, sorted. For work on
/// the workspace as a whole (client stubs the workspace's Cargo needs to
/// resolve), never for choosing what a command runs on.
pub fn all_dirs(workspace_root: &Path) -> Vec<PathBuf> {
    let programs_dir = workspace_root.join("programs");
    let entries = std::fs::read_dir(&programs_dir)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", programs_dir.display())));
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| {
                ui::error(format!(
                    "Cannot read an entry of {}: {e}",
                    programs_dir.display()
                ))
            })
            .path();
        let metadata = std::fs::metadata(&path)
            .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", path.display())));
        if metadata.is_dir() && has_lib(&path) {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs
}

/// `line` with its address replaced when it is `program = "<address>"` and
/// the address differs.
fn replace_address(line: &str, program: &str, address: &str) -> Option<String> {
    let (key, value) = line.split_once('=')?;
    if key.trim().trim_matches('"') != program {
        return None;
    }
    let open = value.find('"')?;
    let close = open + 1 + value[open + 1..].find('"')?;
    if &value[open + 1..close] == address {
        return None;
    }
    Some(format!(
        "{key}={}\"{address}\"{}",
        &value[..open],
        &value[close + 1..]
    ))
}

/// `text` (a Naclac.toml) with the address of `program` set to `address` in
/// every `[programs.<cluster>]` table that lists it, or `None` when every
/// listing already has that address. Only the key `program` itself matches,
/// never a longer name that ends with it.
pub fn with_synced_address(text: &str, program: &str, address: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut in_programs = false;
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_programs = trimmed.starts_with("[programs.");
        } else if in_programs {
            if let Some(updated) = replace_address(line, program, address) {
                changed = true;
                out.push_str(&updated);
                continue;
            }
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

/// Whether `text` (a Naclac.toml) has a `[programs.<cluster>]` table.
pub fn has_programs_table(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim().starts_with("[programs."))
}

/// `text` (a Naclac.toml) with `program = "address"` added as the first entry
/// of its first `[programs.<cluster>]` table, the one nearest the top of the
/// file, or `None` when it has no such table.
pub fn with_added_program(text: &str, program: &str, address: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + program.len() + address.len() + 8);
    let mut added = false;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        if !added && line.trim().starts_with("[programs.") {
            let newline = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            if !line.ends_with('\n') {
                out.push_str(newline);
            }
            out.push_str(&format!("{program} = \"{address}\"{newline}"));
            added = true;
        }
    }
    added.then_some(out)
}

/// The address `declare_id!("...")` declares in a program's `lib.rs`.
pub fn declared_id(lib_code: &str) -> Option<&str> {
    let start = lib_code.find("declare_id!(\"")? + "declare_id!(\"".len();
    let end = lib_code[start..].find("\")")?;
    Some(&lib_code[start..start + end])
}

fn keypair_address(path: &Path) -> String {
    let keypair = keypair::read(path).unwrap_or_else(|e| {
        ui::error(format!(
            "{e} (`naclac build` creates a missing deploy keypair.)"
        ))
    });
    solana_signer::Signer::pubkey(&keypair).to_string()
}

/// Errors unless the program's deploy keypair, its `declare_id!` (when its
/// `lib.rs` has one) and its `table_address` in Naclac.toml are one address.
/// `naclac build` is what makes them agree.
pub fn require_synced_ids(workspace_root: &Path, name: &str, table_address: &str) {
    let keypair_path =
        naclac_client_gen::deploy_dir(workspace_root).join(format!("{name}-keypair.json"));
    let keypair = keypair_address(&keypair_path);

    let lib_path = workspace_root
        .join("programs")
        .join(name)
        .join("src/lib.rs");
    let lib_code = std::fs::read_to_string(&lib_path)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", lib_path.display())));
    let declared = declared_id(&lib_code);

    if keypair != table_address || declared.is_some_and(|declared| declared != table_address) {
        ui::error(format!(
            "The addresses of '{name}' disagree: keypair {keypair}, lib.rs {}, Naclac.toml \
             {table_address}. Run `naclac build` to sync them.",
            declared.unwrap_or("(no declare_id!)")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        declared_id, has_programs_table, listed, select, with_added_program, with_synced_address,
    };

    fn config(text: &str) -> toml::Value {
        toml::from_str(text).unwrap()
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn programs_of_every_table_are_listed_once() {
        let config = config(
            "[programs.localnet]\nb = \"1\"\na = \"2\"\n\n[programs.devnet]\na = \"3\"\nc = \"4\"\n",
        );
        let mut listed = listed(&config);
        listed.sort();
        assert_eq!(listed, ["a", "b", "c"]);
    }

    #[test]
    fn nothing_requested_selects_everything_available() {
        let available = names(&["a", "b"]);
        assert_eq!(select(&available, &[]), available);
    }

    #[test]
    fn requested_programs_are_selected_in_the_order_given() {
        let available = names(&["a", "b", "c"]);
        assert_eq!(select(&available, &names(&["c", "a"])), names(&["c", "a"]));
    }

    const NACLAC_TOML: &str = "[programs.localnet]\nescrow = \"old\" # note\nmy_escrow = \"keep\"\n\n[programs.devnet]\n\"escrow\" = \"old\"\n\n[provider]\nescrow = \"old\"\n";

    #[test]
    fn an_address_is_synced_in_every_programs_table_by_exact_key() {
        let synced = with_synced_address(NACLAC_TOML, "escrow", "new").unwrap();
        assert_eq!(
            synced,
            "[programs.localnet]\nescrow = \"new\" # note\nmy_escrow = \"keep\"\n\n[programs.devnet]\n\"escrow\" = \"new\"\n\n[provider]\nescrow = \"old\"\n"
        );
    }

    #[test]
    fn a_listing_that_already_has_the_address_is_left_alone() {
        assert_eq!(with_synced_address(NACLAC_TOML, "my_escrow", "keep"), None);
        assert_eq!(with_synced_address(NACLAC_TOML, "absent", "new"), None);
    }

    #[test]
    fn a_new_program_goes_into_the_first_programs_table_in_the_file() {
        let text = "[provider]\ncluster = \"devnet\"\n\n[programs.devnet]\na = \"1\"\n\n[programs.localnet]\nb = \"2\"\n";
        assert!(has_programs_table(text));
        assert_eq!(
            with_added_program(text, "c", "3").unwrap(),
            "[provider]\ncluster = \"devnet\"\n\n[programs.devnet]\nc = \"3\"\na = \"1\"\n\n[programs.localnet]\nb = \"2\"\n"
        );
    }

    #[test]
    fn a_workspace_without_a_programs_table_gets_nothing_added() {
        let text = "[provider]\ncluster = \"devnet\"\n";
        assert!(!has_programs_table(text));
        assert_eq!(with_added_program(text, "c", "3"), None);
    }

    #[test]
    fn the_declared_id_is_read_from_lib_rs() {
        assert_eq!(
            declared_id("use x;\ndeclare_id!(\"Abc123\");\n"),
            Some("Abc123")
        );
        assert_eq!(declared_id("fn main() {}"), None);
    }
}
