//! `cargo xtask <cmd>` — the one entry point for repo automation. CI and the
//! pre-commit hook both call this, so "green" is defined in exactly one place.
//!
//! Commands:
//!   check   fmt --check + clippy -D warnings   (fast; the pre-commit gate)
//!   ci      check + tests                       (the full gate CI runs)
//!   setup   install the git pre-commit hook path

use std::process::{exit, Command};

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    let ok = match task.as_str() {
        "check" => run_check(),
        "ci" => run_check() && run_tests(),
        "setup" => run_setup(),
        other => {
            eprintln!("unknown xtask {other:?}. commands: check | ci | setup");
            false
        }
    };
    if !ok {
        exit(1);
    }
}

/// Formatting + lints. Lints are denied so warnings fail the build.
fn run_check() -> bool {
    cargo(&["fmt", "--all", "--", "--check"])
        && cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
}

/// The full test suite, including the architecture-boundary test.
fn run_tests() -> bool {
    cargo(&["test", "--workspace"])
}

/// Point git at the repo's committed hooks so the pre-commit gate runs locally.
fn run_setup() -> bool {
    println!("$ git config core.hooksPath .githooks");
    Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cargo(args: &[&str]) -> bool {
    println!("$ cargo {}", args.join(" "));
    Command::new("cargo")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
