//! Reads the progress cargo prints about itself
//! (`Building [===>   ] 31/187: serde, syn`) so a command can show how far a
//! compile is. Cargo prints it to a pipe too when given [`ENV`]. It is cargo's
//! display, not a documented interface, so a line that does not match is
//! ignored and the caller keeps whatever it showed before.

use {
    crate::{commands::build::strip_ansi, ui},
    colored::Colorize,
    std::io::{self, BufRead, BufReader, Read},
};

/// The environment that makes cargo print its progress line to a pipe.
pub const ENV: [(&str, &str); 2] = [
    ("CARGO_TERM_PROGRESS_WHEN", "always"),
    ("CARGO_TERM_PROGRESS_WIDTH", "200"),
];

/// How far a compile is: `done` of `total` steps, and the crates being
/// compiled right now.
#[derive(Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub active: Vec<String>,
}

/// The progress in `line` if it is one of cargo's `Building [...] N/M: ...`
/// lines. A name cargo cut short with `…` is dropped, and a build script's
/// `(build)`/`(build.rs)` mark is removed from its crate's name.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let clean = strip_ansi(line);
    let after_label = clean.trim().strip_prefix("Building [")?;
    let (_, after_bar) = after_label.split_once(']')?;
    let (counts, names) = after_bar.split_once(':').unwrap_or((after_bar, ""));
    let (done, total) = counts.trim().split_once('/')?;
    let done: usize = done.trim().parse().ok()?;
    let total: usize = total.trim().parse().ok()?;

    let mut names: Vec<&str> = names.split(',').map(str::trim).collect();
    if names.last().is_some_and(|name| name.ends_with('…')) {
        names.pop();
    }
    let mut active: Vec<String> = Vec::new();
    for name in names {
        let crate_name = name.split('(').next().unwrap_or(name).trim();
        if !crate_name.is_empty() && !active.iter().any(|seen| seen == crate_name) {
            active.push(crate_name.to_string());
        }
    }
    Some(Progress {
        done,
        total,
        active,
    })
}

/// A bar, the counts, and up to two of the crates being compiled.
pub fn render(progress: &Progress) -> String {
    let names = progress
        .active
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {:>3}/{}  {}",
        ui::bar(progress.done, progress.total, 24),
        progress.done,
        progress.total,
        names.dimmed()
    )
}

/// Calls `on_update` with every piece of `reader` that ends in `\n` or `\r`
/// (cargo redraws its progress line with `\r`), skipping empty pieces, and
/// with the last piece if the stream ends without a delimiter.
pub fn read_updates(reader: impl Read, mut on_update: impl FnMut(&str)) -> io::Result<()> {
    let mut reader = BufReader::new(reader);
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        let length = chunk.len();
        for &byte in chunk {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    on_update(&String::from_utf8_lossy(&pending));
                    pending.clear();
                }
            } else {
                pending.push(byte);
            }
        }
        reader.consume(length);
    }
    if !pending.is_empty() {
        on_update(&String::from_utf8_lossy(&pending));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, std::io::Cursor};

    #[test]
    fn a_progress_line_gives_its_counts_and_crates() {
        let line = "    Building [=======================> ] 121/124: naclac-lang, probe, naclac-core                        ";
        assert_eq!(
            parse_progress(line),
            Some(Progress {
                done: 121,
                total: 124,
                active: vec!["naclac-lang".into(), "probe".into(), "naclac-core".into()],
            })
        );
    }

    #[test]
    fn a_name_cargo_cut_short_is_dropped_and_build_marks_are_removed() {
        let line = "    Building [                            ] 0/96: quote(build.rs), proc-macro2(build.rs), unicode…";
        let progress = parse_progress(line).unwrap();
        assert_eq!((progress.done, progress.total), (0, 96));
        assert_eq!(progress.active, ["quote", "proc-macro2"]);
    }

    #[test]
    fn a_crate_active_twice_is_listed_once() {
        let line = "    Building [===>   ] 6/12: serde_core(build), serde_core, memchr";
        assert_eq!(
            parse_progress(line).unwrap().active,
            ["serde_core", "memchr"]
        );
    }

    #[test]
    fn ansi_codes_around_the_line_do_not_matter() {
        let line = "\u{1b}[2K\u{1b}[1m    Building\u{1b}[0m [=>  ] 3/10: syn";
        assert_eq!(parse_progress(line).unwrap().done, 3);
    }

    #[test]
    fn a_progress_line_without_crates_is_still_progress() {
        let progress = parse_progress("    Building [=====>  ] 10/12").unwrap();
        assert_eq!((progress.done, progress.total), (10, 12));
        assert!(progress.active.is_empty());
    }

    #[test]
    fn lines_that_are_not_progress_are_ignored() {
        for line in [
            "   Compiling serde v1.0.229",
            "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 57s",
            "    Building [broken",
            "    Building [==>] x/10: a",
            "    Building [==>] 4/y: a",
            "",
        ] {
            assert_eq!(parse_progress(line), None, "{line:?}");
        }
    }

    #[test]
    fn the_rendered_progress_shows_counts_and_at_most_two_crates() {
        let progress = Progress {
            done: 47,
            total: 96,
            active: vec!["solana-address".into(), "syn".into(), "quote".into()],
        };
        let text = strip_ansi(&render(&progress));
        assert!(text.contains("47/96"), "{text}");
        assert!(text.contains("solana-address, syn"), "{text}");
        assert!(!text.contains("quote"), "{text}");
    }

    #[test]
    fn updates_are_split_on_newlines_and_carriage_returns() {
        let mut seen = Vec::new();
        read_updates(Cursor::new(b"a\rb\r\nc\n\nlast".to_vec()), |piece| {
            seen.push(piece.to_string())
        })
        .unwrap();
        assert_eq!(seen, ["a", "b", "c", "last"]);
    }
}
