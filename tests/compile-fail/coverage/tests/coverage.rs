//! Checks that every `Naclac Error:` message the macro crates can emit has a
//! test, so a new error cannot be added without one, and that the compile-fail
//! fixtures are laid out as the runner expects.
//!
//! A message is covered when it appears in a committed
//! `../tests/fail/**/*.stderr` snapshot. A message tested some other way, or
//! that no compile test can reach, is listed in `../exemptions.txt` with the
//! test that covers it or the reason. The source is parsed with `syn`, so
//! messages inside `quote!`, `format!` and `compile_error!` are seen, and
//! everything under `#[cfg(test)]` is skipped. Run with `cargo test` in this
//! directory.

use {
    proc_macro2::{TokenStream, TokenTree},
    quote::ToTokens,
    regex::Regex,
    std::{
        fs,
        path::{Path, PathBuf},
    },
    syn::visit::{self, Visit},
};

const SCANNED: &[&str] = &["derive/src", "syn/src", "core/src", "idl/src", "lang/src"];
const SCOPES: &[&str] = &[
    "solana",
    "pinocchio",
    "bytemuck",
    "borsh",
    "wincode",
    "heap",
    "solana-bytemuck",
    "solana-borsh",
    "solana-wincode",
    "pinocchio-bytemuck",
    "pinocchio-wincode",
    "idl-build",
    "idl-build-bytemuck",
    "idl-build-borsh",
    "idl-build-wincode",
    "no-layout",
    "solana-no-layout",
    "pinocchio-no-layout",
];

/// Scopes whose fail fixtures have no pass counterpart: the corrected program needs an account-data
/// layout feature, which a `no-layout` run deliberately lacks.
const NO_PASS_COUNTERPART: &[&str] = &["no-layout", "solana-no-layout", "pinocchio-no-layout"];
const PREFIX: &str = "Naclac Error: ";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("the repository root exists")
}

fn compile_fail_dir() -> PathBuf {
    repo_root().join("tests/compile-fail")
}

fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            found.extend(files_with_extension(&path, extension));
        } else if path.extension().is_some_and(|ext| ext == extension) {
            found.push(path);
        }
    }
    found.sort();
    found
}

struct Message {
    file: String,
    line: usize,
    text: String,
}

struct Collector {
    file: String,
    messages: Vec<Message>,
}

fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let text = attr.meta.to_token_stream().to_string();
        text.starts_with("cfg") && text.contains("test")
    })
}

impl Collector {
    fn record(&mut self, value: &str, line: usize) {
        let Some(start) = value.find("Naclac Error:") else {
            return;
        };
        let text = value[start..].lines().next().unwrap_or_default().to_string();
        self.messages.push(Message {
            file: self.file.clone(),
            line,
            text,
        });
    }

    /// Records every string in the arguments of a real `compile_error!` call as a
    /// message, whatever it starts with: `naclac-core` and `naclac-lang` word theirs
    /// `naclac-core: ..` rather than `Naclac Error: ..`.
    fn record_every_string(&mut self, tokens: TokenStream) {
        for tree in tokens {
            if let TokenTree::Literal(literal) = tree {
                if let Ok(string) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                    let text = string.value().lines().next().unwrap_or_default().to_string();
                    self.messages.push(Message {
                        file: self.file.clone(),
                        line: literal.span().start().line,
                        text,
                    });
                }
            }
        }
    }

    fn walk(&mut self, tokens: TokenStream) {
        for tree in tokens {
            match tree {
                TokenTree::Group(group) => self.walk(group.stream()),
                TokenTree::Literal(literal) => {
                    if let Ok(string) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        self.record(&string.value(), literal.span().start().line);
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !is_cfg_test(&node.attrs) {
            visit::visit_item_mod(self, node);
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if !is_cfg_test(&node.attrs) {
            visit::visit_item_fn(self, node);
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !is_cfg_test(&node.attrs) {
            visit::visit_item_impl(self, node);
        }
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.record(&node.value(), node.span().start().line);
    }

    /// Reads the strings inside an attribute's argument list, such as
    /// `#[diagnostic::on_unimplemented(message = "..")]`; doc comments are
    /// prose, not messages.
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("doc") {
            return;
        }
        if let syn::Meta::List(list) = &node.meta {
            self.walk(list.tokens.clone());
        }
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.is_ident("compile_error") {
            self.record_every_string(node.tokens.clone());
        } else {
            self.walk(node.tokens.clone());
        }
    }
}

fn source_messages() -> Vec<Message> {
    let root = repo_root();
    let mut messages = Vec::new();
    for crate_src in SCANNED {
        for path in files_with_extension(&root.join(crate_src), "rs") {
            let source = fs::read_to_string(&path).expect("readable source file");
            let file = syn::parse_file(&source)
                .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
            let mut collector = Collector {
                file: path
                    .strip_prefix(&root)
                    .expect("a scanned file is inside the repository")
                    .to_string_lossy()
                    .replace('\\', "/"),
                messages: Vec::new(),
            };
            collector.visit_file(&file);
            messages.extend(collector.messages);
        }
    }
    messages
}

/// The regex for a message as written in the source: `{..}` format
/// placeholders match any text on the line, `{{` and `}}` are literal braces.
fn template_regex(text: &str) -> Regex {
    let mut pattern = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                pattern.push_str(r"\{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                pattern.push_str(r"\}");
            }
            '{' => {
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
                pattern.push_str(".*?");
            }
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
    }
    Regex::new(&pattern).expect("a message template always forms a valid regex")
}

fn snapshots() -> String {
    files_with_extension(&compile_fail_dir().join("tests/fail"), "stderr")
        .iter()
        .map(|path| fs::read_to_string(path).expect("readable snapshot"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace('\r', "")
}

enum Exemption {
    CoveredBy { path: String, test: String },
    Reason(String),
}

struct ExemptionEntry {
    prefix: String,
    kind: Exemption,
    line: usize,
}

fn exemptions() -> Vec<ExemptionEntry> {
    let path = compile_fail_dir().join("exemptions.txt");
    let text = fs::read_to_string(&path).expect("tests/compile-fail/exemptions.txt exists");
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("# ") || line == "#" {
            continue;
        }
        let (prefix, rest) = line.split_once(" || ").unwrap_or_else(|| {
            panic!("exemptions.txt:{}: expected `PREFIX || covered-by PATH::TEST` or `PREFIX || reason TEXT`", index + 1)
        });
        let kind = if let Some(target) = rest.strip_prefix("covered-by ") {
            let (path, test) = target.split_once("::").unwrap_or_else(|| {
                panic!("exemptions.txt:{}: `covered-by` needs PATH::TEST", index + 1)
            });
            Exemption::CoveredBy {
                path: path.trim().to_string(),
                test: test.trim().to_string(),
            }
        } else if let Some(reason) = rest.strip_prefix("reason ") {
            Exemption::Reason(reason.trim().to_string())
        } else {
            panic!("exemptions.txt:{}: expected `covered-by` or `reason` after `||`", index + 1)
        };
        entries.push(ExemptionEntry {
            prefix: prefix.trim().to_string(),
            kind,
            line: index + 1,
        });
    }
    entries
}

struct BaselineEntry {
    file: String,
    prefix: String,
    line: usize,
}

fn baseline() -> Vec<BaselineEntry> {
    let path = compile_fail_dir().join("uncovered-baseline.txt");
    let text = fs::read_to_string(&path).expect("tests/compile-fail/uncovered-baseline.txt exists");
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.starts_with("# ") || line == "#" {
            continue;
        }
        let (file, prefix) = line.split_once(" || ").unwrap_or_else(|| {
            panic!(
                "uncovered-baseline.txt:{}: expected `FILE || START-OF-MESSAGE`",
                index + 1
            )
        });
        entries.push(BaselineEntry {
            file: file.to_string(),
            prefix: prefix.to_string(),
            line: index + 1,
        });
    }
    entries
}

#[test]
fn every_error_message_has_a_test() {
    let messages = source_messages();
    assert!(
        messages.len() > 100,
        "only {} messages were found; the scan is broken",
        messages.len()
    );
    let snapshots = snapshots();
    let entries = exemptions();
    let known_gaps = baseline();
    let mut gap_used = vec![false; known_gaps.len()];
    let root = repo_root();

    let mut problems = Vec::new();
    let mut exempted = vec![false; entries.len()];
    let mut uncovered = Vec::new();

    for message in &messages {
        let in_snapshot = template_regex(&message.text).is_match(&snapshots);
        let body = message.text.strip_prefix(PREFIX).unwrap_or(&message.text);
        let gaps: Vec<usize> = known_gaps
            .iter()
            .enumerate()
            .filter(|(_, gap)| gap.file == message.file && body.starts_with(&gap.prefix))
            .map(|(index, _)| index)
            .collect();
        for &index in &gaps {
            gap_used[index] = true;
            if in_snapshot {
                problems.push(format!(
                    "uncovered-baseline.txt:{}: `{}` is now covered by a snapshot; remove the line",
                    known_gaps[index].line, known_gaps[index].prefix
                ));
            }
        }
        let matching: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| body.starts_with(&entry.prefix))
            .map(|(index, _)| index)
            .collect();
        for &index in &matching {
            exempted[index] = true;
            if in_snapshot {
                problems.push(format!(
                    "exemptions.txt:{}: `{}` is exempted but a snapshot already covers it; remove the entry",
                    entries[index].line, entries[index].prefix
                ));
            }
        }
        if !in_snapshot && matching.is_empty() && gaps.is_empty() {
            uncovered.push(format!("{}:{}  {}", message.file, message.line, message.text));
        }
        if !gaps.is_empty() && !matching.is_empty() {
            problems.push(format!(
                "{}:{}: the message is both in uncovered-baseline.txt and exemptions.txt; remove the baseline line",
                message.file, message.line
            ));
        }
    }

    for (index, gap) in known_gaps.iter().enumerate() {
        if !gap_used[index] {
            problems.push(format!(
                "uncovered-baseline.txt:{}: `{}` in {} matches no message; remove the line",
                gap.line, gap.prefix, gap.file
            ));
        }
    }

    for (index, entry) in entries.iter().enumerate() {
        if !exempted[index] {
            problems.push(format!(
                "exemptions.txt:{}: `{}` matches no message in the source; remove or fix the entry",
                entry.line, entry.prefix
            ));
        }
        if let Exemption::CoveredBy { path, test } = &entry.kind {
            let file = root.join(path);
            match fs::read_to_string(&file) {
                Ok(source) if source.contains(&format!("fn {test}(")) => {}
                Ok(_) => problems.push(format!(
                    "exemptions.txt:{}: {path} has no test named `{test}`",
                    entry.line
                )),
                Err(_) => problems.push(format!(
                    "exemptions.txt:{}: {path} does not exist",
                    entry.line
                )),
            }
        }
        if let Exemption::Reason(reason) = &entry.kind {
            if reason.len() < 20 {
                problems.push(format!(
                    "exemptions.txt:{}: the reason for `{}` is too short to explain anything",
                    entry.line, entry.prefix
                ));
            }
        }
    }

    assert!(
        uncovered.is_empty() && problems.is_empty(),
        "{} error message(s) have no snapshot or exemption:\n{}\n\nexemption problems:\n{}",
        uncovered.len(),
        uncovered.join("\n"),
        problems.join("\n")
    );
}

fn relative_to(base: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(base)
        .expect("a fixture is inside its kind folder")
        .to_path_buf()
}

#[test]
fn fixtures_are_laid_out_as_the_runner_expects() {
    let tests = compile_fail_dir().join("tests");
    let mut problems = Vec::new();

    for kind in ["fail", "pass"] {
        let base = tests.join(kind);
        for dir in fs::read_dir(&base).expect("fixture folder exists") {
            let path = dir.expect("readable directory entry").path();
            if path.is_dir() {
                let name = path.file_name().expect("a folder has a name").to_string_lossy();
                if !SCOPES.contains(&name.as_ref()) {
                    problems.push(format!(
                        "tests/{kind}/{name} is not a scope the runner reads; use one of {SCOPES:?}"
                    ));
                }
            }
        }
    }

    let fail = tests.join("fail");
    let pass = tests.join("pass");
    for snapshot in files_with_extension(&fail, "stderr") {
        if !snapshot.with_extension("rs").exists() {
            problems.push(format!(
                "{} has no fixture next to it",
                relative_to(&tests, &snapshot).display()
            ));
        }
    }
    for fixture in files_with_extension(&fail, "rs") {
        let relative = relative_to(&fail, &fixture);
        if !fixture.with_extension("stderr").exists() {
            problems.push(format!("tests/fail/{} has no .stderr snapshot", relative.display()));
        }
        let exempt_scope = relative
            .components()
            .next()
            .is_some_and(|scope| NO_PASS_COUNTERPART.iter().any(|name| scope.as_os_str() == *name));
        if !exempt_scope && !pass.join(&relative).exists() {
            problems.push(format!(
                "tests/fail/{0} has no valid counterpart at tests/pass/{0}",
                relative.display()
            ));
        }
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
