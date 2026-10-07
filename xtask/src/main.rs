//! Repo tasks, cross-platform, no shell scripts: `cargo xtask <command>`.
//!
//!   dev [--ui]         run the desktop app (Tauri dev, hot reload); --ui = UI only in a browser
//!   build [--debug]    production build: installers for this OS (NSIS + MSI / .app + DMG)
//!   setup              install missing dev tools, npm deps, git hooks + commit template
//!   doctor             show which tools are there, which are missing, how to get them
//!   check [--fast|--ci] the gate (see `check`); --fast = pre-commit subset,
//!                      --ci = also fail if generated files differ from the commit
//!   test               Rust tests (nextest + doctests) and UI tests (vitest)
//!   gen-types          engine-protocol + api-types -> apps/desktop/src/lib/types
//!   tokens             ui-tokens/*.json -> apps/desktop/src/lib/tokens.css
//!   bench              criterion benchmarks (DSP cost per 10 ms block)
//!   coverage           line coverage as HTML (target/llvm-cov/html)
//!   hook <name>        entry points for Claude Code hooks (see .claude/settings.json)

mod hook;
mod tokens;
mod tools;

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
    let flag = |f: &str| args.iter().any(|a| a == f);
    let root = repo_root();
    let result = match args.first().map(String::as_str) {
        Some("dev") => dev(&root, flag("--ui")),
        Some("build") => build(&root, flag("--debug")),
        Some("setup") => tools::setup(&root),
        Some("doctor") => tools::doctor(&root),
        Some("check") => check(&root, flag("--ci"), flag("--fast")),
        Some("test") => test(&root),
        Some("gen-types") => gen_types(&root),
        Some("tokens") => tokens::generate(&root),
        Some("bench") => cargo(&root, &["bench", "-p", "dsp"]),
        Some("coverage") => coverage(&root),
        Some("hook") => hook::run(&root, args.get(1).map(String::as_str)),
        _ => {
            eprintln!(
                "usage: cargo xtask <dev [--ui] | build [--debug] | setup | doctor \
                 | check [--fast|--ci] | test | gen-types | tokens | bench | coverage \
                 | hook <post-edit|session-start>>"
            );
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

/// The gate. Full: everything CI runs. `fast` (git pre-commit): formatting, tokens and clippy
/// only — the tests run on pre-push.
fn check(root: &Path, ci: bool, fast: bool) -> Result {
    tools::require(&["cargo-nextest", "cargo-deny"])?;
    step("cargo fmt --check", || {
        cargo(root, &["fmt", "--all", "--check"])
    })?;
    step("tokens", || tokens::generate(root))?;
    ensure_node_modules(root, ci)?;
    let desktop = root.join(DESKTOP);
    step("prettier --check", || {
        npm(&desktop, &["run", "format:check"])
    })?;
    if !fast {
        step("svelte-check", || npm(&desktop, &["run", "check"]))?;
    }
    let clippy = [
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ];
    step("cargo clippy", || cargo(root, &clippy))?;
    if !fast {
        // Also regenerates the ts-rs bindings (export_bindings_* tests).
        test(root)?;
        step("cargo deny", || {
            cargo(root, &["deny", "--log-level", "error", "check"])
        })?;
    }
    if ci {
        step("generated files committed", || generated_clean(root))?;
    }
    println!("\n✔ check passed{}", if fast { " (fast)" } else { "" });
    Ok(())
}

fn dev(root: &Path, ui_only: bool) -> Result {
    ensure_node_modules(root, false)?;
    tokens::generate(root)?;
    let script = if ui_only { "dev" } else { "app:dev" };
    npm(&root.join(DESKTOP), &["run", script])
}

/// Release build + installers. Tauri runs `npm run build` (Vite) first and writes the
/// bundles to target/<profile>/bundle/<kind>/.
fn build(root: &Path, debug: bool) -> Result {
    ensure_node_modules(root, false)?;
    tokens::generate(root)?;
    let script = if debug {
        "app:build:debug"
    } else {
        "app:build"
    };
    step("tauri build", || npm(&root.join(DESKTOP), &["run", script]))?;

    let profile = if debug { "debug" } else { "release" };
    let bundle_dir = root.join("target").join(profile).join("bundle");
    println!("\ninstallers:");
    let kinds =
        std::fs::read_dir(&bundle_dir).map_err(|e| format!("{}: {e}", bundle_dir.display()))?;
    for kind in kinds.flatten() {
        let Ok(files) = std::fs::read_dir(kind.path()) else {
            continue;
        };
        for file in files.flatten().map(|f| f.path()) {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "exe" | "msi" | "dmg" | "app") {
                let mb = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0) as f64 / 1e6;
                let rel = file.strip_prefix(root).unwrap_or(&file);
                println!("  {}  ({mb:.1} MB)", rel.display());
            }
        }
    }
    Ok(())
}

fn test(root: &Path) -> Result {
    tools::require(&["cargo-nextest"])?;
    step("cargo nextest", || {
        cargo(root, &["nextest", "run", "--workspace", "--no-fail-fast"])
    })?;
    // nextest does not run doctests.
    step("cargo test --doc", || {
        cargo(root, &["test", "--workspace", "--doc", "--quiet"])
    })?;
    ensure_node_modules(root, false)?;
    step("vitest", || npm(&root.join(DESKTOP), &["run", "test"]))
}

fn gen_types(root: &Path) -> Result {
    let args = [
        "test",
        "-p",
        "engine-protocol",
        "-p",
        "api-types",
        "export_",
    ];
    cargo(root, &args)?;
    println!("types written to apps/desktop/src/lib/types");
    Ok(())
}

fn coverage(root: &Path) -> Result {
    tools::require(&["cargo-llvm-cov", "cargo-nextest"])?;
    cargo(
        root,
        &["llvm-cov", "nextest", "--workspace", "--html", "--open"],
    )
}

fn ensure_node_modules(root: &Path, ci: bool) -> Result {
    let desktop = root.join(DESKTOP);
    if ci {
        step("npm ci", || npm(&desktop, &["ci"]))
    } else if !desktop.join("node_modules").exists() {
        step("npm install", || npm(&desktop, &["install"]))
    } else {
        Ok(())
    }
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
    run(Command::new(tools::exe("npm")).current_dir(dir).args(args))
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
