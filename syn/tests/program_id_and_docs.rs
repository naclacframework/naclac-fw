//! An instruction's docs come from the `lib.rs` wrapper when it has any,
//! otherwise from the implementation elsewhere in the crate.

use {naclac_syn::instruction_docs, std::fs};

const LIB_RS: &str = r#"
use naclac_lang::prelude::*;

pub mod handlers;

#[program]
pub mod sample {
    /// Wrapper docs.
    pub fn documented_on_both(ctx: Context<Pinned>) -> Result {
        handlers::documented_on_both(ctx)
    }

    pub fn documented_on_handler_only(ctx: Context<Pinned>) -> Result {
        handlers::documented_on_handler_only(ctx)
    }

    pub fn undocumented(ctx: Context<Pinned>) -> Result {
        handlers::undocumented(ctx)
    }
}
"#;

const HANDLERS_RS: &str = r#"
/// Handler docs that must lose to the wrapper's.
pub fn documented_on_both(ctx: Context<Pinned>) -> Result {
    Ok(())
}

/// Handler-only docs.
pub fn documented_on_handler_only(ctx: Context<Pinned>) -> Result {
    Ok(())
}

pub fn undocumented(ctx: Context<Pinned>) -> Result {
    Ok(())
}
"#;

#[test]
fn instruction_docs_prefer_the_wrapper_then_fall_back_to_the_implementation() {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-instruction-docs-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sample\"\n").unwrap();
    fs::write(dir.join("src/lib.rs"), LIB_RS).unwrap();
    fs::write(dir.join("src/handlers.rs"), HANDLERS_RS).unwrap();
    let names = [
        "documented_on_both".to_string(),
        "documented_on_handler_only".to_string(),
        "undocumented".to_string(),
    ];
    let docs = instruction_docs(&dir, &names);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(docs["documented_on_both"], vec!["Wrapper docs."]);
    assert_eq!(
        docs["documented_on_handler_only"],
        vec!["Handler-only docs."]
    );
    assert!(docs["undocumented"].is_empty());
}
