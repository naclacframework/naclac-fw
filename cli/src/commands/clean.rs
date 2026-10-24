//! `naclac clean` — deletes cargo's `target/` directory in place. Never
//! touches `artifacts/`, which holds the program keypairs (losing one changes
//! that program's on-chain address). Deletes directly rather than moving
//! anything to a staging/trash location first, so cross-filesystem
//! `target-dir` redirects (a different drive, a mounted vhd) can't silently
//! fail the way a `rename`-based move would, and a Ctrl+C mid-clean just
//! leaves some regenerable cache still on disk.

use {
    indicatif::{ProgressBar, ProgressStyle},
    std::{
        fs,
        io::{self, Write},
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    },
};

pub fn execute(hard: bool) {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if hard {
        let has_node = current_dir.join("node_modules").exists();
        let has_dist = current_dir.join("dist").exists();

        let mut extras = String::new();
        if has_node && has_dist {
            extras = "\n   It will also wipe 'node_modules/' and 'dist/'.".to_string();
        } else if has_node {
            extras = "\n   It will also wipe 'node_modules/'.".to_string();
        } else if has_dist {
            extras = "\n   It will also wipe 'dist/'.".to_string();
        }

        println!("⚠️  WARNING: You are executing a HARD clean!");
        println!(
            "   This will permanently delete the entire 'target/' directory.{}",
            extras
        );
        print!("   Are you entirely sure you want to proceed? [y/N]: ");

        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() != "y" {
            println!("🛑 Clean aborted.");
            return;
        }
    }

    println!("🧹 Running safe workspace clean...");

    let target_dir = naclac_client_gen::resolve_target_dir(&current_dir);

    let mut extra_dirs = vec![current_dir.join(".anchor")];
    if hard {
        extra_dirs.push(current_dir.join("node_modules"));
        extra_dirs.push(current_dir.join("dist"));
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let r = cancelled.clone();
    ctrlc::set_handler(move || {
        r.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Deleting files (Press Ctrl+C to stop early — already-deleted cache is safe to leave partial)...");

    let c = cancelled.clone();
    let handle = thread::spawn(move || {
        // One bulk `remove_dir_all`/`remove_file` per entry — the same
        // primitive `cargo clean` itself relies on — rather than a manual
        // per-file walk. Cancellation is checked between entries, not within
        // one; a bulk removal already in flight runs to completion rather
        // than stopping mid-file.
        fn wipe_path(path: &Path) {
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }

        fn clean_dir_entries(dir: &Path, c: &Arc<AtomicBool>) -> bool {
            let Ok(entries) = fs::read_dir(dir) else {
                return true;
            };
            for entry in entries.flatten() {
                if c.load(Ordering::SeqCst) {
                    return false;
                }
                wipe_path(&entry.path());
            }
            true
        }

        let mut completed = true;
        if target_dir.exists() {
            completed = clean_dir_entries(&target_dir, &c);
            if completed && hard {
                let _ = fs::remove_dir(&target_dir);
            }
        }
        if completed {
            for dir in &extra_dirs {
                if c.load(Ordering::SeqCst) {
                    completed = false;
                    break;
                }
                if dir.exists() {
                    wipe_path(dir);
                }
            }
        }
        completed
    });

    while !handle.is_finished() {
        pb.tick();
        thread::sleep(Duration::from_millis(50));
    }

    let completed = handle.join().unwrap_or(false);

    if !completed {
        pb.finish_with_message(
            "🛑 Clean stopped early by user — deleted cache is gone, anything not yet reached is untouched. Safe to re-run.",
        );
    } else if hard {
        pb.finish_with_message("✨ Workspace completely wiped!");
    } else {
        pb.finish_with_message("✨ Workspace cleaned securely!\n🔒 (Program keypairs in artifacts/deploy are never touched).\n💡 Run `naclac clean --hard` to also remove the target directory itself and node_modules/dist.");
    }
}
