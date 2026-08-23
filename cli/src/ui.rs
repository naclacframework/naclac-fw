//! Shared terminal-output primitives for every `naclac` subcommand: one
//! consistent, low-noise way to show what's currently running, report
//! success, and report errors — instead of each command hand-rolling its
//! own `eprintln!` formatting and emoji choices.

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fmt::Display;
use std::time::{Duration, Instant};

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
        bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
        );
        bar.enable_steady_tick(Duration::from_millis(80));
        bar.set_message(msg.to_string());
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
        let elapsed = format_duration(self.start.elapsed());
        self.bar.finish_and_clear();
        eprintln!("{} {} {}", "✓".green().bold(), elapsed.dimmed(), msg);
    }

    pub fn fail(self, msg: impl Display) -> ! {
        let elapsed = format_duration(self.start.elapsed());
        self.bar.finish_and_clear();
        eprintln!("{} {} {}", "✗".red().bold(), elapsed.dimmed(), msg);
        std::process::exit(1);
    }
}

pub fn success(msg: impl Display) {
    eprintln!("{} {}", "✓".green().bold(), msg);
}

pub fn warn(msg: impl Display) {
    eprintln!("{} {}", "!".yellow().bold(), msg);
}

/// Prints a single-line, non-fatal error (caller keeps running — e.g. one
/// failure inside a per-program loop). For the fatal case, use [`error`].
pub fn error_line(msg: impl Display) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

/// Prints a single-line error and exits the process with status 1.
pub fn error(msg: impl Display) -> ! {
    error_line(msg);
    std::process::exit(1);
}

/// A plain status line for something that just happened, with no
/// success/failure verdict attached (e.g. "Deploying to devnet...").
pub fn info(msg: impl Display) {
    eprintln!("{}", msg);
}

/// The final "everything this command set out to do is done" line — the one
/// line a multi-step command (e.g. `naclac build` across several programs)
/// prints once at the very end, distinct from each step's own ✓ so it reads
/// as the overall verdict rather than another step result.
pub fn milestone(msg: impl Display) {
    eprintln!("{} {}", "★".cyan().bold(), msg.to_string().bold());
}
