//! Vanity program addresses for `naclac init` and `naclac add`: asks whether
//! the program's address should be random or start/end with chosen
//! characters, and grinds a keypair for it on every CPU core. The keypair is
//! an ordinary ed25519 keypair from the same library the CLI signs with, so
//! nothing but the search itself differs from a random address.

use {
    crate::ui,
    clap::Args,
    dialoguer::{Confirm, Input, Select},
    solana_keypair::Keypair,
    solana_signer::Signer,
    std::{
        fmt::{self, Display},
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc::{self, RecvTimeoutError},
        },
        thread,
        time::{Duration, Instant},
    },
};

const BASE58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// The most characters a pattern may fix in all (start and end together).
pub const MAX_PATTERN_CHARS: usize = 6;

/// From this many fixed characters on, the search is warned about.
const WARN_FROM_CHARS: usize = 5;

/// An estimated search longer than this is confirmed before it starts when the
/// pattern was typed at the prompt.
const CONFIRM_ABOVE: Duration = Duration::from_secs(5 * 60);

const BENCHMARK: Duration = Duration::from_secs(1);
const BATCH: u64 = 256;

/// The command-line way to answer the address question, shared by `init` and
/// `add`. Without any of these, the question is asked at a prompt.
#[derive(Args, Debug)]
pub struct AddressArgs {
    /// Grind a vanity program address: `Prefix...`, `...Suffix` or
    /// `Prefix...Suffix`, at most 6 characters in all
    #[arg(long, conflicts_with = "random_address")]
    pub vanity: Option<String>,
    /// Match the vanity pattern ignoring case
    #[arg(long = "vanity-ignore-case", requires = "vanity")]
    pub vanity_ignore_case: bool,
    /// Use a random program address; skips the vanity question
    #[arg(long = "random-address")]
    pub random_address: bool,
}

/// What an address has to start and end with.
pub struct Pattern {
    prefix: String,
    suffix: String,
    ignore_case: bool,
}

impl Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}...{}", self.prefix, self.suffix)?;
        if self.ignore_case {
            write!(f, " (any case)")?;
        }
        Ok(())
    }
}

impl Pattern {
    /// Parses `Prefix...Suffix`, `Prefix...` or `...Suffix`.
    pub fn parse(text: &str, ignore_case: bool) -> Result<Pattern, String> {
        let Some((prefix, suffix)) = text.split_once("...") else {
            return Err(format!(
                "The vanity pattern '{text}' must contain `...` between the start and the end \
                 you want, e.g. `Nac...`, `...xyz` or `Nac...xyz`."
            ));
        };
        Pattern::new(prefix, suffix, ignore_case)
    }

    fn new(prefix: &str, suffix: &str, ignore_case: bool) -> Result<Pattern, String> {
        let length = prefix.chars().count() + suffix.chars().count();
        if length == 0 {
            return Err("The vanity pattern fixes no characters.".to_string());
        }
        if length > MAX_PATTERN_CHARS {
            return Err(format!(
                "The vanity pattern fixes {length} characters; at most {MAX_PATTERN_CHARS} are \
                 allowed (each extra character makes the search about 58 times longer)."
            ));
        }
        let pattern = Pattern {
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            ignore_case,
        };
        for c in pattern.prefix.chars().chain(pattern.suffix.chars()) {
            if variants(c, ignore_case).is_empty() {
                return Err(format!(
                    "'{c}' cannot appear in an address: base58 has no `0`, `O`, `I` or `l`, \
                     and only letters and digits."
                ));
            }
        }
        Ok(pattern)
    }

    fn length(&self) -> usize {
        self.prefix.chars().count() + self.suffix.chars().count()
    }

    fn matches(&self, address: &str) -> bool {
        let (Some(start), Some(end)) = (
            address.get(..self.prefix.len()),
            address.get(address.len().saturating_sub(self.suffix.len())..),
        ) else {
            return false;
        };
        if self.ignore_case {
            start.eq_ignore_ascii_case(&self.prefix) && end.eq_ignore_ascii_case(&self.suffix)
        } else {
            start == self.prefix && end == self.suffix
        }
    }

    /// How many random addresses it takes on average to find one that matches.
    /// The first character is not uniform in base58: a 44-character address
    /// can only begin with `1` to `J`, and later characters appear only in the
    /// shorter addresses.
    fn expected_attempts(&self) -> f64 {
        let mut chance = 1.0;
        for (index, c) in self.prefix.chars().enumerate() {
            let options = variants(c, self.ignore_case);
            chance *= if index == 0 {
                options.iter().map(|&v| first_char_chance(v)).sum::<f64>()
            } else {
                options.len() as f64 / 58.0
            };
        }
        for c in self.suffix.chars() {
            chance *= variants(c, self.ignore_case).len() as f64 / 58.0;
        }
        1.0 / chance
    }
}

/// The base58 characters `c` stands for: itself, and with `ignore_case` its
/// other case; only those that exist in base58.
fn variants(c: char, ignore_case: bool) -> Vec<char> {
    let mut options = vec![c];
    if ignore_case {
        for other in [c.to_ascii_lowercase(), c.to_ascii_uppercase()] {
            if !options.contains(&other) {
                options.push(other);
            }
        }
    }
    options.retain(|v| BASE58.contains(*v));
    options
}

/// The chance that a random address begins with `c`, from the ranges base58
/// gives each length: a 44-character address (94% of them) begins with a digit
/// from index 1 to 17, a 43-character one with any index, and `1` stands for a
/// zero first byte.
fn first_char_chance(c: char) -> f64 {
    let Some(index) = BASE58.find(c) else {
        return 0.0;
    };
    if index == 0 {
        return 1.0 / 256.0;
    }
    let long = (43.0 * 58f64.ln() - 256.0 * 2f64.ln()).exp();
    let short = long / 58.0;
    short
        + match index {
            1..=16 => long,
            17 => 1.0 - 17.0 * long,
            _ => 0.0,
        }
}

/// Resolves the address question for a new program: `None` for a random
/// address, or a keypair whose address matches the chosen pattern, ground
/// here before anything is created. With `--vanity` the pattern is taken from
/// the flag; with `--random-address` there is nothing to ask; otherwise the
/// question is asked at a prompt.
pub fn keypair_for(args: &AddressArgs) -> Option<Keypair> {
    let (pattern, asked) = match (&args.vanity, args.random_address) {
        (Some(text), _) => (
            Pattern::parse(text, args.vanity_ignore_case).unwrap_or_else(|e| ui::error(e)),
            false,
        ),
        (None, true) => return None,
        (None, false) => (prompt_pattern()?, true),
    };
    Some(grind(&pattern, asked))
}

fn prompt_failed(error: dialoguer::Error) -> ! {
    ui::error(format!(
        "Cannot ask for the program address: {error}. Choose without a prompt with \
         --random-address or --vanity <PATTERN>."
    ))
}

fn prompt_pattern() -> Option<Pattern> {
    let choice = Select::with_theme(&ui::DialogTheme)
        .with_prompt("Program address")
        .default(0)
        .items([
            "Random address",
            "Vanity address (starts and/or ends with characters you choose)",
        ])
        .interact()
        .unwrap_or_else(|e| prompt_failed(e));
    if choice == 0 {
        return None;
    }
    loop {
        let prefix = ask("Address starts with (empty for any)");
        let suffix = ask("Address ends with (empty for any)");
        let ignore_case = Confirm::with_theme(&ui::DialogTheme)
            .with_prompt("Ignore upper/lower case")
            .default(false)
            .interact()
            .unwrap_or_else(|e| prompt_failed(e));
        match Pattern::new(&prefix, &suffix, ignore_case) {
            Ok(pattern) => return Some(pattern),
            Err(e) => ui::warn(e),
        }
    }
}

fn ask(prompt: &str) -> String {
    Input::<String>::with_theme(&ui::DialogTheme)
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .unwrap_or_else(|e| prompt_failed(e))
        .trim()
        .to_string()
}

fn grind(pattern: &Pattern, asked: bool) -> Keypair {
    let threads = thread::available_parallelism()
        .unwrap_or_else(|e| ui::error(format!("Cannot tell how many CPU cores there are: {e}")))
        .get();

    ui::kv("vanity", pattern);
    let step = ui::Step::start("Measuring how fast this machine searches");
    let rate = benchmark(threads);
    step.clear();

    let expected = pattern.expected_attempts();
    let estimate = Duration::from_secs_f64(expected / rate);
    ui::kv(
        "search",
        format!(
            "about {} addresses to try · {} cores · {}/s · usually {}",
            compact(expected),
            threads,
            compact(rate),
            duration_text(estimate)
        ),
    );
    if pattern.length() >= WARN_FROM_CHARS {
        ui::warn(format!(
            "{} characters can take a long time; each character fewer is about 58 times faster",
            pattern.length()
        ));
    }
    if asked && estimate > CONFIRM_ABOVE {
        let go = Confirm::with_theme(&ui::DialogTheme)
            .with_prompt(format!(
                "This usually takes {}. Search anyway",
                duration_text(estimate)
            ))
            .default(false)
            .interact()
            .unwrap_or_else(|e| prompt_failed(e));
        if !go {
            ui::error("Cancelled. Nothing was created.");
        }
    }

    let step = ui::Step::start("Searching for the address");
    let started = Instant::now();
    let stop = AtomicBool::new(false);
    let tried = AtomicU64::new(0);
    let (sender, receiver) = mpsc::channel::<Keypair>();
    let is_match = |address: &str| pattern.matches(address);

    let found = thread::scope(|scope| {
        for _ in 0..threads {
            let sender = sender.clone();
            let (is_match, stop, tried) = (&is_match, &stop, &tried);
            scope.spawn(move || work(is_match, None, stop, tried, &sender));
        }
        drop(sender);
        let outcome = loop {
            match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(keypair) => break Ok(keypair),
                Err(RecvTimeoutError::Timeout) => {
                    let done = tried.load(Ordering::Relaxed);
                    let seconds = started.elapsed().as_secs_f64().max(0.001);
                    step.set_message(format!(
                        "Searching for the address  {} tried · {}/s · {}",
                        compact(done as f64),
                        compact(done as f64 / seconds),
                        duration_text(started.elapsed())
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    break Err("the search stopped without finding an address".to_string())
                }
            }
        };
        stop.store(true, Ordering::Relaxed);
        outcome
    });

    let keypair = match found {
        Ok(keypair) => keypair,
        Err(e) => step.fail(e),
    };
    let address = keypair.pubkey().to_string();
    if !pattern.matches(&address) {
        step.fail(format!("The address {address} does not match {pattern}."));
    }
    match Keypair::try_from(keypair.to_bytes().as_slice()) {
        Ok(read_back) if read_back.pubkey() == keypair.pubkey() => {}
        _ => step.fail("The ground keypair does not read back as the same keypair."),
    }
    step.done(format!(
        "found {address} after {} addresses",
        compact(tried.load(Ordering::Relaxed) as f64)
    ));
    keypair
}

/// Addresses per second over about a second on `threads` cores.
fn benchmark(threads: usize) -> f64 {
    let stop = AtomicBool::new(false);
    let tried = AtomicU64::new(0);
    let (sender, _receiver) = mpsc::channel::<Keypair>();
    let never = |_: &str| false;
    let started = Instant::now();
    thread::scope(|scope| {
        for _ in 0..threads {
            let sender = sender.clone();
            let (never, stop, tried) = (&never, &stop, &tried);
            scope.spawn(move || work(never, Some(started + BENCHMARK), stop, tried, &sender));
        }
    });
    tried.load(Ordering::Relaxed) as f64 / started.elapsed().as_secs_f64()
}

/// One search thread: generates keypairs until `stop` is set or `until`
/// passes, counting them in `tried`, and sends the first whose address
/// `is_match`.
fn work(
    is_match: &(dyn Fn(&str) -> bool + Sync),
    until: Option<Instant>,
    stop: &AtomicBool,
    tried: &AtomicU64,
    found: &mpsc::Sender<Keypair>,
) {
    let mut pending = 0;
    while !stop.load(Ordering::Relaxed) {
        let keypair = Keypair::new();
        pending += 1;
        if pending == BATCH {
            tried.fetch_add(pending, Ordering::Relaxed);
            pending = 0;
            if until.is_some_and(|deadline| Instant::now() >= deadline) {
                return;
            }
        }
        if is_match(&keypair.pubkey().to_string()) {
            tried.fetch_add(pending, Ordering::Relaxed);
            if found.send(keypair).is_ok() {
                stop.store(true, Ordering::Relaxed);
            }
            return;
        }
    }
    tried.fetch_add(pending, Ordering::Relaxed);
}

fn compact(count: f64) -> String {
    if count >= 1e9 {
        format!("{:.1}B", count / 1e9)
    } else if count >= 1e6 {
        format!("{:.1}M", count / 1e6)
    } else if count >= 1e3 {
        format!("{:.0}k", count / 1e3)
    } else {
        format!("{count:.0}")
    }
}

fn duration_text(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds < 90.0 {
        format!("{seconds:.0}s")
    } else if seconds < 5400.0 {
        format!("{:.0} min", seconds / 60.0)
    } else {
        format!("{:.1} h", seconds / 3600.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_gives_its_start_and_end() {
        let pattern = Pattern::parse("Na...xyz", false).unwrap();
        assert!(pattern.matches("Na11111111111111111111111111111111111111xyz"));
        assert!(!pattern.matches("na11111111111111111111111111111111111111xyz"));
        assert!(!pattern.matches("Na11111111111111111111111111111111111111xyZ"));
        let start = Pattern::parse("Nac...", false).unwrap();
        assert!(start.matches("Nac111111111111111111111111111111111111111"));
        let end = Pattern::parse("...xyz", false).unwrap();
        assert!(end.matches("111111111111111111111111111111111111111xyz"));
    }

    #[test]
    fn ignoring_case_matches_either_case() {
        let pattern = Pattern::parse("nac...", true).unwrap();
        assert!(pattern.matches("NAC111111111111111111111111111111111111111"));
        assert!(pattern.matches("nAc111111111111111111111111111111111111111"));
    }

    #[test]
    fn a_pattern_needs_the_dots_and_at_least_one_character() {
        assert!(Pattern::parse("Nac", false).is_err());
        assert!(Pattern::parse("...", false).is_err());
    }

    #[test]
    fn a_pattern_may_fix_six_characters_and_not_seven() {
        assert!(Pattern::parse("Na...cxy", false).is_ok());
        assert!(Pattern::parse("Nac...xyz", false).is_ok());
        assert!(Pattern::parse("Nacxyzw...", false).is_err());
        assert!(Pattern::parse("Nacx...xyz", false).is_err());
    }

    #[test]
    fn a_character_base58_does_not_have_is_an_error() {
        for pattern in ["0ab...", "Oab...", "Iab...", "lab...", "a_b..."] {
            assert!(Pattern::parse(pattern, false).is_err(), "{pattern}");
        }
        assert!(Pattern::parse("lab...", true).is_ok());
    }

    #[test]
    fn each_character_makes_the_search_about_58_times_longer() {
        let one = Pattern::parse("...a", false).unwrap().expected_attempts();
        let three = Pattern::parse("...abc", false).unwrap().expected_attempts();
        assert!((one - 58.0).abs() < 1e-6);
        assert!((three / one - 58.0 * 58.0).abs() < 1e-3);
    }

    #[test]
    fn ignoring_case_makes_the_search_shorter() {
        let exact = Pattern::parse("...ab", false).unwrap().expected_attempts();
        let any_case = Pattern::parse("...ab", true).unwrap().expected_attempts();
        assert!((exact / any_case - 4.0).abs() < 1e-6);
    }

    #[test]
    fn the_first_character_chances_follow_real_addresses() {
        let samples = 20_000;
        let mut early = 0;
        let mut late = 0;
        for _ in 0..samples {
            let address = Keypair::new().pubkey().to_string();
            match BASE58.find(address.chars().next().unwrap()).unwrap() {
                1..=16 => early += 1,
                18.. => late += 1,
                _ => {}
            }
        }
        let early_model: f64 = BASE58.chars().skip(1).take(16).map(first_char_chance).sum();
        let late_model: f64 = BASE58.chars().skip(18).map(first_char_chance).sum();
        assert!((early as f64 / samples as f64 - early_model).abs() < 0.02);
        assert!((late as f64 / samples as f64 - late_model).abs() < 0.01);
    }

    #[test]
    fn a_search_finds_a_short_pattern() {
        let pattern = Pattern::parse("...a", false).unwrap();
        let stop = AtomicBool::new(false);
        let tried = AtomicU64::new(0);
        let (sender, receiver) = mpsc::channel();
        let is_match = |address: &str| pattern.matches(address);
        work(&is_match, None, &stop, &tried, &sender);
        let keypair = receiver.try_recv().unwrap();
        assert!(keypair.pubkey().to_string().ends_with('a'));
    }
}
