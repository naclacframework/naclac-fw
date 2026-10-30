//! Shared terminal-output primitives for every `naclac` subcommand: one
//! consistent, low-noise way to show what's currently running, report
//! success, and report errors — instead of each command hand-rolling its
//! own `eprintln!` formatting and emoji choices. The brand pink is the only
//! accent: ✓ stays green, ✗ red and ! yellow, since those read at a glance.

use {
    colored::{ColoredString, Colorize},
    indicatif::{ProgressBar, ProgressStyle},
    std::{
        fmt::Display,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Mutex, PoisonError,
        },
        time::{Duration, Instant},
    },
};

/// The pink of the naclac logo.
const BRAND: (u8, u8, u8) = (226, 68, 133);

/// The first and last four characters of an address or signature.
pub fn short(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 9 {
        return text.to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Text in the brand pink.
pub fn brand(text: impl Display) -> ColoredString {
    text.to_string().truecolor(BRAND.0, BRAND.1, BRAND.2)
}

/// The spinner of the running [`Step`], if any. Every line printed by this
/// module goes through it, so a line never overwrites the spinner.
static ACTIVE_BAR: Mutex<Option<ProgressBar>> = Mutex::new(None);

/// Whether the user asked for the detail a command normally summarizes
/// (`--show-output`).
static SHOW_OUTPUT: AtomicBool = AtomicBool::new(false);

/// How many spaces every line and spinner is indented by; see [`section`].
static INDENT: AtomicUsize = AtomicUsize::new(0);

pub fn set_show_output(on: bool) {
    SHOW_OUTPUT.store(on, Ordering::Relaxed);
}

pub fn show_output() -> bool {
    SHOW_OUTPUT.load(Ordering::Relaxed)
}

pub fn set_indent(spaces: usize) {
    INDENT.store(spaces, Ordering::Relaxed);
}

fn pad() -> String {
    " ".repeat(INDENT.load(Ordering::Relaxed))
}

fn set_active(bar: Option<ProgressBar>) {
    *ACTIVE_BAR.lock().unwrap_or_else(PoisonError::into_inner) = bar;
}

fn active() -> Option<ProgressBar> {
    ACTIVE_BAR
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Prints one indented line to stderr above the running spinner, if there is
/// one.
fn emit(line: impl Display) {
    let line = format!("{}{line}", pad());
    match active() {
        Some(bar) => bar.suspend(|| eprintln!("{line}")),
        None => eprintln!("{line}"),
    }
}

/// Replaces the running spinner's message; does nothing without a spinner.
pub fn working(msg: impl Display) {
    if let Some(bar) = active() {
        bar.set_message(msg.to_string());
    }
}

/// Formats a duration the way every timed result line in this module does:
/// sub-second as whole milliseconds, otherwise seconds to one decimal place.
pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

/// A timed result line: the mark, the duration right-aligned so a column of
/// results lines up, then the message.
fn result_line(mark: ColoredString, elapsed: Duration, msg: impl Display) -> String {
    format!(
        "{}{} {} {}",
        pad(),
        mark,
        format!("{:>6}", format_duration(elapsed)).dimmed(),
        msg
    )
}

/// An in-place spinner for a step that takes visible time (a subprocess, a
/// network call, a build). `done`/`fail` clear the spinner and print exactly
/// one timed result line in its place, so a step never leaves more than one
/// line behind regardless of how long it ran.
pub struct Step {
    bar: ProgressBar,
    start: Instant,
}

impl Step {
    pub fn start(msg: impl Display) -> Self {
        let bar = ProgressBar::new_spinner();
        let frames: Vec<String> = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
            .chars()
            .map(|frame| brand(frame).to_string())
            .chain(std::iter::once(" ".to_string()))
            .collect();
        let frames: Vec<&str> = frames.iter().map(String::as_str).collect();
        bar.set_style(
            ProgressStyle::with_template(&format!("{}{{spinner}} {{msg}}", pad()))
                .unwrap()
                .tick_strings(&frames),
        );
        bar.enable_steady_tick(Duration::from_millis(80));
        bar.set_message(msg.to_string());
        set_active(Some(bar.clone()));
        Step {
            bar,
            start: Instant::now(),
        }
    }

    pub fn set_message(&self, msg: impl Display) {
        self.bar.set_message(msg.to_string());
    }

    /// Prints a line above the spinner without corrupting its redraw —
    /// for pass-through output (e.g. a subprocess's own warnings/errors)
    /// that must appear in the normal scrollback while the spinner keeps
    /// running underneath it.
    pub fn print_above(&self, line: impl Display) {
        self.bar.suspend(|| eprintln!("{}", line));
    }

    pub fn done(self, msg: impl Display) {
        self.bar.finish_and_clear();
        set_active(None);
        eprintln!(
            "{}",
            result_line("✓".green().bold(), self.start.elapsed(), msg)
        );
    }

    pub fn fail(self, msg: impl Display) -> ! {
        self.failed(msg);
        std::process::exit(1);
    }

    /// Ends the step with a timed failure line and returns, for a caller that
    /// carries on or exits itself.
    pub fn failed(self, msg: impl Display) {
        self.bar.finish_and_clear();
        set_active(None);
        eprintln!(
            "{}",
            result_line("✗".red().bold(), self.start.elapsed(), msg)
        );
    }

    /// Ends the step and prints nothing, for a caller that reports the
    /// failure itself.
    pub fn clear(self) {
        self.bar.finish_and_clear();
        set_active(None);
    }
}

/// The opening line of a command: the brand wordmark, the command, and where
/// it runs, after a blank line.
pub fn header(command: impl Display, target: impl Display) {
    emit("");
    emit(format!(
        "{} {} {} {} {}",
        brand("naclac").bold(),
        "›".dimmed(),
        command.to_string().bold(),
        "·".dimmed(),
        target
    ));
}

/// A dimmed label and its value on one aligned row, below a [`header`].
pub fn kv(key: &str, value: impl Display) {
    emit(format!("  {} {}", format!("{key:<7}").dimmed(), value));
}

/// Starts a block of output about one item (a program): a blank line and a
/// pink `▸ name` heading, then indents everything after it by two spaces
/// until [`end_section`].
pub fn section(name: impl Display, detail: impl Display) {
    set_indent(0);
    emit("");
    emit(format!(
        "{} {}  {}",
        brand("▸").bold(),
        brand(name).bold(),
        detail.to_string().dimmed()
    ));
    set_indent(2);
}

pub fn end_section() {
    set_indent(0);
}

pub fn success(msg: impl Display) {
    emit(format!("{} {}", "✓".green().bold(), msg));
}

pub fn warn(msg: impl Display) {
    emit(format!("{} {}", "!".yellow().bold(), msg));
}

/// Prints a single-line, non-fatal error (caller keeps running — e.g. one
/// failure inside a per-program loop). For the fatal case, use [`error`].
pub fn error_line(msg: impl Display) {
    emit(format!("{} {}", "✗".red().bold(), msg));
}

/// Prints a single-line error and exits the process with status 1.
pub fn error(msg: impl Display) -> ! {
    error_line(msg);
    std::process::exit(1);
}

/// A plain status line for something that just happened, with no
/// success/failure verdict attached (e.g. "Deploying to devnet...").
pub fn info(msg: impl Display) {
    emit(msg);
}

/// The final "everything this command set out to do is done" line — the one
/// line a multi-step command (e.g. `naclac build` across several programs)
/// prints once at the very end, distinct from each step's own ✓ so it reads
/// as the overall verdict rather than another step result.
pub fn milestone(msg: impl Display) {
    set_indent(0);
    emit("");
    emit(format!("{} {}", brand("★").bold(), msg.to_string().bold()));
}
