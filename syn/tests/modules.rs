//! Pins `naclac_syn::modules` to rustc's module rules: file modules (`x.rs`,
//! `x/mod.rs`), inline modules, `#[path]` at file level and inside an inline
//! module, undeclared files, the crate root named by `[lib] path`, and
//! visibility from the crate root.

use {
    naclac_syn::modules::{crate_modules, crate_root, crate_root_of, CrateModule},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn sample_crate(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-modules-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    write(
        &dir,
        "Cargo.toml",
        r#"
        [package]
        name = "sample"
        [features]
        default = ["on"]
        on = ["also_on"]
        also_on = []
        off = []
        "#,
    );
    write(
        &dir,
        "src/lib.rs",
        r#"
        pub mod flat;
        pub mod dir;
        #[path = "far/away.rs"]
        pub mod moved;
        #[cfg(feature = "off")]
        pub mod gated_off;
        #[cfg(all(feature = "also_on", not(feature = "off")))]
        pub mod gated_on;
        mod private_top;
        pub mod outer {
            pub struct InOuter;
            mod hidden {
                pub struct InHidden;
            }
            pub mod open {
                #[path = "pathed_inline.rs"]
                pub mod pathed;
            }
        }
        "#,
    );
    write(&dir, "src/flat.rs", "pub mod child; pub struct InFlat;");
    write(&dir, "src/flat/child.rs", "pub struct InFlatChild;");
    write(&dir, "src/dir/mod.rs", "pub struct InDir;");
    write(&dir, "src/far/away.rs", "pub mod near; pub struct InMoved;");
    write(&dir, "src/far/near.rs", "pub struct InMovedChild;");
    write(&dir, "src/gated_off.rs", "pub struct InGatedOff;");
    write(&dir, "src/gated_on.rs", "pub struct InGatedOn; #[cfg(feature = \"off\")] pub struct ItemOff; #[cfg(target_os = \"solana\")] pub struct UnknownPredicate;");
    write(&dir, "src/private_top.rs", "pub struct InPrivateTop;");
    write(
        &dir,
        "src/outer/open/pathed_inline.rs",
        "pub struct InPathedInline;",
    );
    write(&dir, "src/stray.rs", "pub struct InStray;");
    dir
}

fn structs(modules: &[CrateModule]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = modules
        .iter()
        .flat_map(|m| {
            m.items.iter().filter_map(move |item| match item {
                syn::Item::Struct(s) => Some((m.path.clone(), s.ident.to_string())),
                _ => None,
            })
        })
        .collect();
    out.sort();
    out
}

fn module<'a>(modules: &'a [CrateModule], path: &str) -> &'a CrateModule {
    modules.iter().find(|m| m.path == path).unwrap()
}

#[test]
fn follows_rustc_module_rules_and_ignores_undeclared_files() {
    let dir = sample_crate("rules");
    let modules = crate_modules(&dir);
    let found = structs(&modules);
    let _ = fs::remove_dir_all(&dir);

    let expect = |path: &str, name: &str| (path.to_string(), name.to_string());
    for entry in [
        expect("crate::flat", "InFlat"),
        expect("crate::flat::child", "InFlatChild"),
        expect("crate::dir", "InDir"),
        expect("crate::moved", "InMoved"),
        expect("crate::moved::near", "InMovedChild"),
        expect("crate::gated_off", "InGatedOff"),
        expect("crate::outer", "InOuter"),
        expect("crate::outer::hidden", "InHidden"),
        expect("crate::outer::open::pathed", "InPathedInline"),
    ] {
        assert!(found.contains(&entry), "missing {entry:?} in {found:?}");
    }
    assert!(!found.iter().any(|(_, name)| name == "InStray"));
}

fn bare_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-root-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let payload = std::panic::catch_unwind(f).unwrap_err();
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[test]
fn a_manifest_without_lib_path_uses_cargos_default_root() {
    let dir = bare_dir("default");
    write(&dir, "Cargo.toml", "[package]\nname = \"sample\"\n");
    let root = crate_root(&dir);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(root, dir.join("src/lib.rs"));
}

#[test]
fn lib_path_names_the_crate_root() {
    let dir = bare_dir("lib-path");
    write(&dir, "Cargo.toml", "[lib]\npath = \"src/entry.rs\"\n");
    let root = crate_root(&dir);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(root, dir.join("src/entry.rs"));
}

#[test]
fn a_binary_being_compiled_has_the_root_its_bin_entry_names() {
    let dir = bare_dir("bin-path");
    write(
        &dir,
        "Cargo.toml",
        "[[bin]]\nname = \"other\"\npath = \"src/other.rs\"\n\n\
         [[bin]]\nname = \"fixture\"\npath = \"tests/fixture.rs\"\n",
    );
    let root = crate_root_of(&dir, Some("fixture"));
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(root, dir.join("tests/fixture.rs"));
}

#[test]
fn a_binary_without_a_bin_entry_is_an_error() {
    let dir = bare_dir("bin-missing");
    write(&dir, "Cargo.toml", "[package]\nname = \"sample\"\n");
    let target = dir.clone();
    let message = panic_message(move || {
        crate_root_of(&target, Some("fixture"));
    });
    let _ = fs::remove_dir_all(&dir);
    assert!(message.contains("`fixture`"), "{message}");
    assert!(message.contains("`[[bin]]` entry"), "{message}");
}

#[test]
fn a_missing_manifest_is_an_error() {
    let dir = bare_dir("no-manifest");
    let target = dir.clone();
    let message = panic_message(move || {
        crate_root(&target);
    });
    let _ = fs::remove_dir_all(&dir);
    assert!(message.contains("cannot read"), "{message}");
    assert!(message.contains("Cargo.toml"), "{message}");
}

#[test]
fn an_invalid_manifest_is_an_error() {
    let dir = bare_dir("bad-toml");
    write(&dir, "Cargo.toml", "[package\nname = ");
    let target = dir.clone();
    let message = panic_message(move || {
        crate_root(&target);
    });
    let _ = fs::remove_dir_all(&dir);
    assert!(message.contains("is not valid TOML"), "{message}");
}

#[test]
fn a_lib_entry_of_the_wrong_type_is_an_error() {
    for (tag, manifest, expected) in [
        ("lib-not-table", "lib = 3\n", "`lib` in"),
        (
            "path-not-string",
            "[lib]\npath = 3\n",
            "`path` under `[lib]`",
        ),
    ] {
        let dir = bare_dir(tag);
        write(&dir, "Cargo.toml", manifest);
        let target = dir.clone();
        let message = panic_message(move || {
            crate_root(&target);
        });
        let _ = fs::remove_dir_all(&dir);
        assert!(message.contains(expected), "{tag}: {message}");
    }
}

#[test]
fn an_unreadable_or_unparseable_crate_root_is_an_error() {
    let dir = bare_dir("missing-root");
    write(&dir, "Cargo.toml", "[package]\nname = \"sample\"\n");
    let target = dir.clone();
    let missing = panic_message(move || {
        crate_modules(&target);
    });
    write(&dir, "src/lib.rs", "pub fn broken( {");
    let target = dir.clone();
    let unparseable = panic_message(move || {
        crate_modules(&target);
    });
    let _ = fs::remove_dir_all(&dir);
    assert!(missing.contains("cannot read the crate root"), "{missing}");
    assert!(unparseable.contains("does not parse"), "{unparseable}");
}

#[test]
fn visibility_from_the_crate_root() {
    let dir = sample_crate("visibility");
    let modules = crate_modules(&dir);
    let _ = fs::remove_dir_all(&dir);

    assert!(module(&modules, "crate::private_top").visible_from_root);
    assert!(module(&modules, "crate::outer::open").visible_from_root);
    assert!(module(&modules, "crate::outer::open::pathed").visible_from_root);
    assert!(!module(&modules, "crate::outer::hidden").visible_from_root);
    assert_eq!(module(&modules, "crate::outer::hidden").depth, 2);
}
