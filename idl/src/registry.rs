//! What a program's items register for its IDL, and the checks run over them
//! when the `idl-build` binary assembles the IDL. Each `#[account]`,
//! `#[event]`, `#[error_code]` and `#[constant]` submits one [`IdlFragment`]
//! from its own macro expansion, so the compiler decides which items exist
//! (their `cfg`s, their modules) and nothing reads the crate's source.

pub use crate::idl_type::Types;
use {
    crate::{IdlConstant, IdlError, IdlEvent},
    std::collections::{BTreeMap, BTreeSet},
};

/// The discriminator map of the accounts: type name to bytes.
pub type Discriminators = BTreeMap<String, Vec<u8>>;

pub enum IdlFragment {
    /// An `#[account]` type: inserts its own definition and discriminator, and
    /// the defined types it reaches into the third map.
    Account {
        name: &'static str,
        build: fn(&mut Types, &mut Discriminators, &mut Types),
    },
    /// An `#[event]` type: returns its event, filling in the types it reaches.
    Event {
        name: &'static str,
        build: fn(&mut Types) -> IdlEvent,
    },
    /// An `#[error_code]` enum.
    Errors {
        name: &'static str,
        build: fn() -> Vec<IdlError>,
    },
    /// A `#[constant]`.
    Constant {
        name: &'static str,
        build: fn() -> IdlConstant,
    },
}

inventory::collect!(IdlFragment);

/// Everything the registered fragments describe.
#[derive(Default)]
pub struct Collected {
    pub account_types: Types,
    pub account_discriminators: Discriminators,
    pub defined_types: Types,
    pub events: Vec<IdlEvent>,
    pub errors: Vec<IdlError>,
    pub constants: Vec<IdlConstant>,
}

fn clash(kind: &str, name: &str) -> String {
    format!(
        "Naclac Error: two IDL {kind} items are both named `{name}`. The IDL identifies them by \
         name only; rename one."
    )
}

/// Runs every registered fragment, then checks the result. Panics with every
/// problem found, one per line: a name used twice in a kind, an error code used
/// by two `#[error_code]` enums, and account or event discriminators that clash
/// or are invalid (see `naclac_syn::discriminator::validate_category`).
pub fn collect() -> Collected {
    let mut collected = Collected::default();
    let mut problems: Vec<String> = Vec::new();
    let mut account_names = BTreeSet::new();
    let mut event_names = BTreeSet::new();
    let mut error_enums = BTreeSet::new();
    let mut constant_names = BTreeSet::new();

    for fragment in inventory::iter::<IdlFragment> {
        match fragment {
            IdlFragment::Account { name, build } => {
                if !account_names.insert(*name) {
                    problems.push(clash("account", name));
                }
                build(
                    &mut collected.account_types,
                    &mut collected.account_discriminators,
                    &mut collected.defined_types,
                );
            }
            IdlFragment::Event { name, build } => {
                if !event_names.insert(*name) {
                    problems.push(clash("event", name));
                }
                let event = build(&mut collected.defined_types);
                collected.events.push(event);
            }
            IdlFragment::Errors { name, build } => {
                if !error_enums.insert(*name) {
                    problems.push(clash("error", name));
                }
                collected.errors.extend(build());
            }
            IdlFragment::Constant { name, build } => {
                if !constant_names.insert(*name) {
                    problems.push(clash("constant", name));
                }
                collected.constants.push(build());
            }
        }
    }

    let mut codes: BTreeMap<u32, &str> = BTreeMap::new();
    for error in &collected.errors {
        if let Some(other) = codes.insert(error.code, &error.name) {
            problems.push(format!(
                "Naclac Error: error code {} is used by both `{other}` and `{}`. Give one \
                 `#[error_code]` enum its own range with `#[error_code(offset = N)]`.",
                error.code, error.name
            ));
        }
    }

    let account_entries: Vec<naclac_syn::discriminator::DiscriminatorEntry> = collected
        .account_discriminators
        .iter()
        .map(
            |(name, bytes)| naclac_syn::discriminator::DiscriminatorEntry {
                name: name.clone(),
                bytes: bytes.clone(),
            },
        )
        .collect();
    let event_entries: Vec<naclac_syn::discriminator::DiscriminatorEntry> = collected
        .events
        .iter()
        .map(|event| naclac_syn::discriminator::DiscriminatorEntry {
            name: event.name.clone(),
            bytes: event.discriminator.clone(),
        })
        .collect();
    for (kind, entries) in [
        (
            naclac_syn::discriminator::DiscriminatorKind::Account,
            account_entries,
        ),
        (
            naclac_syn::discriminator::DiscriminatorKind::Event,
            event_entries,
        ),
    ] {
        if let Err(errors) = naclac_syn::discriminator::validate_category(kind, &entries) {
            problems.extend(errors);
        }
    }

    if !problems.is_empty() {
        panic!("{}", problems.join("\n"));
    }
    collected
}
