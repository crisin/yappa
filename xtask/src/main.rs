//! Repo tasks, cross-platform, no shell scripts: `cargo xtask <command>`.
//!
//!   check [--ci]   the gate: fmt, tokens, UI typecheck, clippy -D warnings, tests;
//!                  --ci also fails if generated files differ from what is committed
//!   gen-types      engine-protocol + api-types -> apps/desktop/src/lib/types/*.ts
//!   tokens         ui-tokens/*.json -> apps/desktop/src/lib/tokens.css

mod tokens;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Files produced by xtask/ts-rs that are committed and must match their sources.
const GENERATED: &[&str] = &[
    "apps/desktop/src/lib/types",
    "apps/desktop/src/lib/tokens.css",
];
const DESKTOP: &str = "apps/desktop";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root();
    let result = match args.first().map(String::as_str) {
        Some("check") => check(&root, args.iter().any(|a| a == "--ci")),
        Some("gen-types") => gen_types(&root),
        Some("tokens") => tokens::generate(&root),
        _ => {
            eprintln!("usage: cargo xtask <check [--ci] | gen-types | tokens>");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nxtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the repo root")
        .into()
}

type Result<T = ()> = std::result::Result<T, String>;

fn check(root: &Path, ci: bool) -> Result {
    step("cargo fmt --check", || {
        cargo(root, &["fmt", "--all", "--check"])
    })?;
    step("tokens", || tokens::generate(root))?;
    let desktop = root.join(DESKTOP);
    if !desktop.join("node_modules").exists() || ci {
        step("npm install", || {
            npm(&desktop, &[if ci { "ci" } else { "install" }])
        })?;
    }
    step("ui typecheck", || npm(&desktop, &["run", "check"]))?;
    let clippy = [
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ];
    step("cargo clippy", || cargo(root, &clippy))?;
    // Also regenerates the ts-rs bindings (export_bindings_* tests).
    step("cargo test", || cargo(root, &["test", "--workspace"]))?;
    if ci {
        step("generated files committed", || generated_clean(root))?;
    }
    println!("\n✔ check passed");
    Ok(())
}

fn gen_types(root: &Path) -> Result {
    cargo(
        root,
        &[
            "test",
            "-p",
            "engine-protocol",
            "-p",
            "api-types",
            "export_bindings",
        ],
    )?;
    println!("types written to apps/desktop/src/lib/types");
    Ok(())
}

fn generated_clean(root: &Path) -> Result {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--"])
        .args(GENERATED)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    let dirty = String::from_utf8_lossy(&out.stdout);
    if dirty.trim().is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated files differ from the commit — run `cargo xtask check` and commit:\n{dirty}"
        ))
    }
}

fn step(name: &str, f: impl FnOnce() -> Result) -> Result {
    println!("── {name}");
    f().map_err(|e| format!("{name} failed: {e}"))
}

fn cargo(dir: &Path, args: &[&str]) -> Result {
    run(Command::new(env!("CARGO")).current_dir(dir).args(args))
}

fn npm(dir: &Path, args: &[&str]) -> Result {
    // npm is a .cmd shim on Windows; Command does not resolve those by itself.
    let bin = if cfg!(windows) { "npm.cmd" } else { "npm" };
    run(Command::new(bin).current_dir(dir).args(args))
}

fn run(cmd: &mut Command) -> Result {
    let status = cmd
        .status()
        .map_err(|e| format!("could not start {:?}: {e}", cmd.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} exited with {status}", cmd.get_program()))
    }
}
