//! Shared, single-source-of-truth helpers for values that must match
//! byte-for-byte between the code a macro actually compiles into a program
//! and any other Naclac tool (the IDL's own runtime) that separately needs to
//! know the same value. Kept here so there is exactly one implementation to
//! get right.

use {
    sha2::{Digest, Sha256},
    std::path::Path,
};

/// Width of a discriminator that is derived from the item's name.
pub const DEFAULT_DISCRIMINATOR_LEN: usize = 8;

/// Longest discriminator a user-supplied override may specify.
pub const MAX_DISCRIMINATOR_LEN: usize = 8;

/// Which kind of item a discriminator identifies. Uniqueness, width and
/// zero-value rules apply within one kind only: an instruction, an account
/// and an event are decoded from three different byte streams, so equal bytes
/// across kinds are harmless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscriminatorKind {
    Instruction,
    Account,
    Event,
}

impl DiscriminatorKind {
    pub fn hash_prefix(self) -> &'static str {
        match self {
            DiscriminatorKind::Instruction => "global",
            DiscriminatorKind::Account => "account",
            DiscriminatorKind::Event => "event",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DiscriminatorKind::Instruction => "instruction",
            DiscriminatorKind::Account => "account",
            DiscriminatorKind::Event => "event",
        }
    }
}

/// One item's resolved discriminator, as fed to `validate_category`.
#[derive(Clone, Debug)]
pub struct DiscriminatorEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Computes the 8-byte discriminator for a named item: `sha256("<prefix>:<name>")[0..8]`.
/// - Instructions: `prefix = "global"`
/// - Accounts:     `prefix = "account"`
/// - Events:       `prefix = "event"`
pub fn compute_discriminator(prefix: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{prefix}:{name}");
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// The discriminator an item actually uses: its override bytes when it has
/// one, otherwise the name-derived default. Every consumer must resolve
/// through this function so they cannot disagree.
pub fn resolve_discriminator(
    kind: DiscriminatorKind,
    name: &str,
    override_bytes: Option<&[u8]>,
) -> Vec<u8> {
    match override_bytes {
        Some(bytes) => bytes.to_vec(),
        None => compute_discriminator(kind.hash_prefix(), name).to_vec(),
    }
}

/// Name under which the reserved event-CPI dispatch tag is derived. Every
/// consumer (the `#[program]` dispatcher, `emit_cpi!`, the IDL, client-gen)
/// must obtain the tag via `event_dispatch_tag` so they cannot disagree.
pub const EVENT_DISPATCH_NAME: &str = "__event_dispatch";

/// The reserved instruction-category tag that marks a self-CPI event
/// payload, truncated to the program's instruction-discriminator `width`.
pub fn event_dispatch_tag(width: usize) -> Vec<u8> {
    let full = resolve_discriminator(DiscriminatorKind::Instruction, EVENT_DISPATCH_NAME, None);
    full[..width.min(full.len())].to_vec()
}

fn parse_byte(int: &syn::LitInt) -> syn::Result<u8> {
    if !matches!(int.suffix(), "" | "u8") {
        return Err(syn::Error::new_spanned(
            int,
            "Naclac Error: discriminator bytes must be plain integer literals (or `u8`-suffixed), each in 0..=255.",
        ));
    }
    int.base10_parse::<u8>().map_err(|_| {
        syn::Error::new_spanned(
            int,
            "Naclac Error: each discriminator byte must be an integer literal in 0..=255.",
        )
    })
}

/// Parses the value of a `discriminator = <value>` override: a single integer
/// literal (a 1-byte discriminator) or an array literal of 1 to
/// `MAX_DISCRIMINATOR_LEN` integer literals. Anything else, including const
/// expressions, is an error, so that every consumer reads identical source
/// tokens and resolves identical bytes.
pub fn parse_override_expr(expr: &syn::Expr) -> syn::Result<Vec<u8>> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => Ok(vec![parse_byte(int)?]),
        syn::Expr::Array(array) => {
            if array.elems.is_empty() || array.elems.len() > MAX_DISCRIMINATOR_LEN {
                return Err(syn::Error::new_spanned(
                    array,
                    format!(
                        "Naclac Error: a custom discriminator must be 1 to {MAX_DISCRIMINATOR_LEN} bytes long."
                    ),
                ));
            }
            array
                .elems
                .iter()
                .map(|elem| match elem {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(int),
                        ..
                    }) => parse_byte(int),
                    other => Err(syn::Error::new_spanned(
                        other,
                        "Naclac Error: discriminator bytes must be integer literals.",
                    )),
                })
                .collect()
        }
        other => Err(syn::Error::new_spanned(
            other,
            "Naclac Error: a custom discriminator must be an integer literal (one byte) or an array of integer literals, e.g. `discriminator = 7` or `discriminator = [1, 2, 3]`.",
        )),
    }
}

/// Arguments accepted by `#[component(..)]` and `#[event(..)]`.
#[derive(Debug, Default)]
pub struct MacroArgs {
    pub alloc: bool,
    pub discriminator: Option<Vec<u8>>,
}

/// Parses the comma-separated argument list of `#[component(..)]` (pass
/// `allow_alloc = false`) or `#[event(..)]` (`allow_alloc = true`). Empty
/// tokens are valid and yield the defaults. Unknown or repeated arguments are
/// errors.
pub fn parse_macro_args(
    tokens: proc_macro2::TokenStream,
    allow_alloc: bool,
) -> syn::Result<MacroArgs> {
    use syn::parse::Parser;

    let mut args = MacroArgs::default();
    if tokens.is_empty() {
        return Ok(args);
    }
    let metas = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
        .parse2(tokens)?;
    for meta in metas {
        match meta {
            syn::Meta::Path(path) if allow_alloc && path.is_ident("alloc") => {
                if args.alloc {
                    return Err(syn::Error::new_spanned(
                        path,
                        "Naclac Error: duplicate `alloc` argument.",
                    ));
                }
                args.alloc = true;
            }
            syn::Meta::NameValue(nv) if nv.path.is_ident("discriminator") => {
                if args.discriminator.is_some() {
                    return Err(syn::Error::new_spanned(
                        nv.path,
                        "Naclac Error: duplicate `discriminator` argument.",
                    ));
                }
                args.discriminator = Some(parse_override_expr(&nv.value)?);
            }
            other => {
                let accepted = if allow_alloc {
                    "`alloc` and `discriminator = ..`"
                } else {
                    "`discriminator = ..`"
                };
                return Err(syn::Error::new_spanned(
                    other,
                    format!(
                        "Naclac Error: unknown argument; the accepted arguments are {accepted}."
                    ),
                ));
            }
        }
    }
    Ok(args)
}

/// `parse_macro_args` applied to an attribute as written in source: a
/// bare `#[component]` / `#[event]` yields the defaults.
pub fn macro_args_of(attr: &syn::Attribute, allow_alloc: bool) -> syn::Result<MacroArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        _ => proc_macro2::TokenStream::new(),
    };
    parse_macro_args(tokens, allow_alloc)
}

/// Reads the standalone `#[discriminator = <value>]` attribute from a
/// `#[program]` handler function's attributes. `Ok(None)` when absent.
pub fn parse_handler_override(attrs: &[syn::Attribute]) -> syn::Result<Option<Vec<u8>>> {
    let mut found: Option<Vec<u8>> = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("discriminator")) {
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "Naclac Error: duplicate `#[discriminator = ..]` attribute.",
            ));
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            return Err(syn::Error::new_spanned(
                attr,
                "Naclac Error: write the override as `#[discriminator = <value>]`, e.g. `#[discriminator = 7]` or `#[discriminator = [1, 2, 3]]`.",
            ));
        };
        found = Some(parse_override_expr(&nv.value)?);
    }
    Ok(found)
}

/// Checks one kind's discriminators for the properties dispatch and account
/// loading rely on. Returns every violation found.
/// - All entries share one width (an entry without an override counts as
///   `DEFAULT_DISCRIMINATOR_LEN`). This is what makes a discriminator being a
///   byte-prefix of another impossible.
/// - No two entries have identical bytes.
/// - An account discriminator is never all-zero at any width: an
///   all-zero prefix is how uninitialized accounts are recognized.
/// - An instruction discriminator of more than one byte is never all-zero.
pub fn validate_category(
    kind: DiscriminatorKind,
    entries: &[DiscriminatorEntry],
) -> Result<(), Vec<String>> {
    let label = kind.label();
    let mut errors = Vec::new();

    if let Some(first) = entries.first() {
        if entries.iter().any(|e| e.bytes.len() != first.bytes.len()) {
            let widths: Vec<String> = entries
                .iter()
                .map(|e| format!("`{}` ({} bytes)", e.name, e.bytes.len()))
                .collect();
            errors.push(format!(
                "Naclac Error: every {label} in a program must use the same discriminator width, but found: {}. An {label} without a custom discriminator uses the default {DEFAULT_DISCRIMINATOR_LEN}-byte hash.",
                widths.join(", ")
            ));
        }
    }

    for (i, entry) in entries.iter().enumerate() {
        let all_zero = entry.bytes.iter().all(|b| *b == 0);
        let forbidden_zero = match kind {
            DiscriminatorKind::Account => all_zero,
            DiscriminatorKind::Instruction => all_zero && entry.bytes.len() > 1,
            DiscriminatorKind::Event => false,
        };
        if forbidden_zero {
            errors.push(format!(
                "Naclac Error: {label} `{}` has an all-zero discriminator {:?}, which is not allowed for this kind.",
                entry.name, entry.bytes
            ));
        }
        for other in &entries[i + 1..] {
            if other.bytes == entry.bytes {
                errors.push(format!(
                    "Naclac Error: {label}s `{}` and `{}` have the same discriminator {:?}.",
                    entry.name, other.name, entry.bytes
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Every account (`#[component]`) and event (`#[event]`) discriminator
/// declared in a crate's modules, in a stable name order.
#[derive(Debug, Default)]
pub struct CrateDiscriminators {
    pub accounts: Vec<DiscriminatorEntry>,
    pub events: Vec<DiscriminatorEntry>,
}

fn collect_struct_discriminators(items: &[syn::Item], out: &mut CrateDiscriminators) {
    for item in items {
        if let syn::Item::Struct(item_struct) = item {
            for attr in &item_struct.attrs {
                let (kind, allow_alloc) = if attr.path().is_ident("component") {
                    (DiscriminatorKind::Account, false)
                } else if attr.path().is_ident("event") {
                    (DiscriminatorKind::Event, true)
                } else {
                    continue;
                };
                // A malformed argument list is reported by the item's own
                // macro at its own span; skipping it here only avoids a
                // second, less precise error for the same mistake.
                let Ok(args) = macro_args_of(attr, allow_alloc) else {
                    continue;
                };
                let name = item_struct.ident.to_string();
                let entry = DiscriminatorEntry {
                    bytes: resolve_discriminator(kind, &name, args.discriminator.as_deref()),
                    name,
                };
                match kind {
                    DiscriminatorKind::Account => out.accounts.push(entry),
                    _ => out.events.push(entry),
                }
            }
        }
    }
}

/// Resolves the discriminator of every `#[component]` and `#[event]` struct
/// in every module of the crate (see `crate::modules`). Items generated by
/// other macros are not visible to this scan.
pub fn scan_crate_discriminators(crate_dir: &Path) -> CrateDiscriminators {
    let mut found = CrateDiscriminators::default();
    for module in crate::modules::crate_modules(crate_dir) {
        collect_struct_discriminators(&module.items, &mut found);
    }

    found.accounts.sort_by(|a, b| a.name.cmp(&b.name));
    found.events.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Computes every variant's real, fully-resolved discriminant value,
/// mirroring Rust's own enum discriminant rule exactly: starts at -1; an
/// explicit `= N` resets the running value to `N`; otherwise the running
/// value increments by 1 from the previous variant. Getting this wrong for
/// an enum with an explicit discriminant would silently validate the wrong
/// tag byte on-chain, or report the wrong value in the IDL.
pub fn variant_discriminants<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
) -> Vec<i128> {
    let mut last_value: i128 = -1;
    variants
        .into_iter()
        .map(|v| {
            if let Some((_, expr)) = &v.discriminant {
                last_value = parse_discriminant_expr(expr);
            } else {
                last_value = last_value.wrapping_add(1);
            }
            last_value
        })
        .collect()
}

/// Detects an enum's explicit `#[repr(uN)]`/`#[repr(iN)]` discriminant type,
/// if written — deliberately ignores a non-integer repr modifier like
/// `#[repr(C)]`/`#[repr(packed)]`/`#[repr(align(N))]` written alone, since
/// none of those alone commits to any particular discriminant width. Callers
/// that need a concrete tag type (e.g. `#[defined_type]`'s zero-copy enum
/// branch, which builds a `bytemuck::CheckedBitPattern` layout sized to
/// whatever this returns) must apply the same `u8` default on `None` that
/// they use for deciding whether to add their own `#[repr(u8)]` — treating
/// this as "no repr at all" when the enum actually has e.g. bare `#[repr(C)]`
/// would silently build a `Bits` layout narrower than the real compiled
/// enum's size (a bare `#[repr(C)]` data-carrying enum's discriminant is
/// platform-defined, not 1 byte).
pub fn detect_enum_repr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("repr") {
            return None;
        }
        let mut found = None;
        let _ = attr.parse_nested_meta(|meta| {
            for candidate in [
                "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
            ] {
                if meta.path.is_ident(candidate) {
                    found = Some(candidate.to_string());
                }
            }
            Ok(())
        });
        found
    })
}

fn parse_discriminant_expr(expr: &syn::Expr) -> i128 {
    match expr {
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => -parse_discriminant_expr(expr),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => int
            .base10_parse()
            .expect("naclac-syn: enum discriminant literal must fit in i128"),
        _ => panic!("naclac-syn: enum discriminant must be a literal integer expression"),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs};

    fn expr(src: &str) -> syn::Expr {
        syn::parse_str(src).expect("test source must parse as an expression")
    }

    fn entry(name: &str, bytes: &[u8]) -> DiscriminatorEntry {
        DiscriminatorEntry {
            name: name.to_string(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn override_accepts_single_literal_and_arrays() {
        assert_eq!(parse_override_expr(&expr("7")).unwrap(), vec![7]);
        assert_eq!(parse_override_expr(&expr("7u8")).unwrap(), vec![7]);
        assert_eq!(parse_override_expr(&expr("0xFF")).unwrap(), vec![255]);
        assert_eq!(
            parse_override_expr(&expr("[1, 2, 3]")).unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            parse_override_expr(&expr("[1, 2, 3, 4, 5, 6, 7, 8]")).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn override_rejects_invalid_values() {
        for bad in [
            "256",
            "-1",
            "7u64",
            "[]",
            "[1, 2, 3, 4, 5, 6, 7, 8, 9]",
            "[1, 300]",
            "SOME_CONST",
            "[1, SOME_CONST]",
            "\"ab\"",
            "1 + 1",
        ] {
            assert!(
                parse_override_expr(&expr(bad)).is_err(),
                "`{bad}` must be rejected"
            );
        }
    }

    #[test]
    fn macro_args_parse_alloc_and_discriminator_in_any_order() {
        let none = parse_macro_args(quote::quote! {}, true).unwrap();
        assert!(!none.alloc && none.discriminator.is_none());

        let alloc = parse_macro_args(quote::quote! { alloc }, true).unwrap();
        assert!(alloc.alloc && alloc.discriminator.is_none());

        let disc = parse_macro_args(quote::quote! { discriminator = [1, 2] }, false).unwrap();
        assert!(!disc.alloc);
        assert_eq!(disc.discriminator, Some(vec![1, 2]));

        let a = parse_macro_args(quote::quote! { alloc, discriminator = 9 }, true).unwrap();
        let b = parse_macro_args(quote::quote! { discriminator = 9, alloc }, true).unwrap();
        assert!(a.alloc && b.alloc);
        assert_eq!(a.discriminator, Some(vec![9]));
        assert_eq!(b.discriminator, Some(vec![9]));
    }

    #[test]
    fn macro_args_reject_unknown_repeated_and_disallowed() {
        assert!(parse_macro_args(quote::quote! { alloc }, false).is_err());
        assert!(parse_macro_args(quote::quote! { bogus }, true).is_err());
        assert!(parse_macro_args(quote::quote! { alloc, alloc }, true).is_err());
        assert!(
            parse_macro_args(quote::quote! { discriminator = 1, discriminator = 2 }, true).is_err()
        );
        assert!(parse_macro_args(quote::quote! { discriminator(1) }, true).is_err());
        assert!(parse_macro_args(quote::quote! { discriminator = FOO }, true).is_err());
    }

    #[test]
    fn handler_override_reads_only_the_name_value_form() {
        let none: syn::ItemFn = syn::parse_quote! { pub fn f() {} };
        assert_eq!(parse_handler_override(&none.attrs).unwrap(), None);

        let one: syn::ItemFn = syn::parse_quote! { #[discriminator = 5] pub fn f() {} };
        assert_eq!(parse_handler_override(&one.attrs).unwrap(), Some(vec![5]));

        let arr: syn::ItemFn = syn::parse_quote! { #[discriminator = [1, 2]] pub fn f() {} };
        assert_eq!(
            parse_handler_override(&arr.attrs).unwrap(),
            Some(vec![1, 2])
        );

        let list: syn::ItemFn = syn::parse_quote! { #[discriminator(5)] pub fn f() {} };
        assert!(parse_handler_override(&list.attrs).is_err());

        let dup: syn::ItemFn =
            syn::parse_quote! { #[discriminator = 1] #[discriminator = 2] pub fn f() {} };
        assert!(parse_handler_override(&dup.attrs).is_err());
    }

    #[test]
    fn resolve_uses_override_or_the_hashed_default() {
        assert_eq!(
            resolve_discriminator(DiscriminatorKind::Account, "Vault", None),
            compute_discriminator("account", "Vault").to_vec()
        );
        assert_eq!(
            resolve_discriminator(DiscriminatorKind::Event, "Vault", Some(&[1, 2])),
            vec![1, 2]
        );
        assert_eq!(
            resolve_discriminator(DiscriminatorKind::Instruction, "deposit", None).len(),
            DEFAULT_DISCRIMINATOR_LEN
        );
    }

    #[test]
    fn validate_accepts_a_consistent_category() {
        let entries = [entry("a", &[1]), entry("b", &[2]), entry("c", &[3])];
        assert!(validate_category(DiscriminatorKind::Instruction, &entries).is_ok());
        assert!(validate_category(DiscriminatorKind::Account, &entries).is_ok());
        assert!(validate_category(DiscriminatorKind::Instruction, &[]).is_ok());
    }

    #[test]
    fn validate_rejects_duplicates_within_a_kind() {
        let entries = [entry("a", &[1, 2]), entry("b", &[1, 2])];
        for kind in [
            DiscriminatorKind::Instruction,
            DiscriminatorKind::Account,
            DiscriminatorKind::Event,
        ] {
            assert!(validate_category(kind, &entries).is_err());
        }
    }

    #[test]
    fn validate_rejects_mixed_widths_including_default_vs_override() {
        let mixed = [entry("a", &[1]), entry("b", &[2, 3])];
        assert!(validate_category(DiscriminatorKind::Instruction, &mixed).is_err());

        let default_vs_short = [
            entry("a", &compute_discriminator("global", "a")),
            entry("b", &[9]),
        ];
        assert!(validate_category(DiscriminatorKind::Instruction, &default_vs_short).is_err());

        let default_vs_full = [
            entry("a", &compute_discriminator("global", "a")),
            entry("b", &[9, 9, 9, 9, 9, 9, 9, 9]),
        ];
        assert!(validate_category(DiscriminatorKind::Instruction, &default_vs_full).is_ok());
    }

    #[test]
    fn validate_zero_rules_differ_by_kind() {
        let zero_one = [entry("a", &[0])];
        assert!(validate_category(DiscriminatorKind::Instruction, &zero_one).is_ok());
        assert!(validate_category(DiscriminatorKind::Account, &zero_one).is_err());
        assert!(validate_category(DiscriminatorKind::Event, &zero_one).is_ok());

        let zero_many = [entry("a", &[0, 0, 0])];
        assert!(validate_category(DiscriminatorKind::Instruction, &zero_many).is_err());
        assert!(validate_category(DiscriminatorKind::Account, &zero_many).is_err());
        assert!(validate_category(DiscriminatorKind::Event, &zero_many).is_ok());

        let leading_zero_only = [entry("a", &[0, 0, 1])];
        assert!(validate_category(DiscriminatorKind::Account, &leading_zero_only).is_ok());
    }

    #[test]
    fn scan_finds_components_and_events_including_inline_modules() {
        let dir = std::env::temp_dir().join(format!(
            "naclac-syn-disc-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sample\"\n").unwrap();
        fs::write(
            src.join("lib.rs"),
            r#"
            #[component(discriminator = [4, 5])]
            pub struct Vault { pub a: u64 }

            #[component]
            pub struct Plain { pub a: u64 }

            #[event(alloc, discriminator = 9)]
            pub struct Deposited { pub a: u64 }

            mod inner {
                #[event]
                pub struct Withdrawn { pub a: u64 }
            }
            "#,
        )
        .unwrap();

        let found = scan_crate_discriminators(&dir);
        let _ = fs::remove_dir_all(&dir);

        let account = |name: &str| {
            found
                .accounts
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.bytes.clone())
        };
        let event = |name: &str| {
            found
                .events
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.bytes.clone())
        };

        assert_eq!(account("Vault"), Some(vec![4, 5]));
        assert_eq!(
            account("Plain"),
            Some(compute_discriminator("account", "Plain").to_vec())
        );
        assert_eq!(event("Deposited"), Some(vec![9]));
        assert_eq!(
            event("Withdrawn"),
            Some(compute_discriminator("event", "Withdrawn").to_vec())
        );
    }
}
