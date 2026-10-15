//! Pins `naclac_syn::modules` to rustc's module rules: file modules (`x.rs`,
//! `x/mod.rs`), inline modules, `#[path]` at file level and inside an inline
//! module, undeclared files, `cfg` evaluation over default features, and
//! visibility from the crate root.

use naclac_syn::modules::{active_crate_modules, crate_modules, default_features, CrateModule};
use std::fs;
use std::path::{Path, PathBuf};

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

#[test]
fn default_features_are_closed_over_the_features_table() {
    let dir = sample_crate("features");
    let features = default_features(&dir);
    let _ = fs::remove_dir_all(&dir);
    assert!(features.contains("on"));
    assert!(features.contains("also_on"));
    assert!(!features.contains("off"));
}

#[test]
fn active_modules_drop_what_default_features_compile_out() {
    let dir = sample_crate("active");
    let features = default_features(&dir);
    let found = structs(&active_crate_modules(&dir, &features));
    let _ = fs::remove_dir_all(&dir);

    let names: Vec<&str> = found.iter().map(|(_, name)| name.as_str()).collect();
    assert!(!names.contains(&"InGatedOff"));
    assert!(!names.contains(&"ItemOff"));
    assert!(names.contains(&"InGatedOn"));
    assert!(names.contains(&"UnknownPredicate"));
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
