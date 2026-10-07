//! The dev toolbox, defined once for the Windows PC, the MacBook and CI.
//! `cargo xtask doctor` reports, `cargo xtask setup` installs what can be installed.

use crate::Result;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, PartialEq)]
enum Need {
    /// `cargo xtask check` fails without it.
    Required,
    /// Needed from a given spike on.
    From(&'static str),
    Optional,
}

enum Install {
    /// `cargo binstall <crate>` (prebuilt binary, falls back to compiling).
    Binstall(&'static str),
    /// Not installable from here; how to get it.
    Hint(&'static str),
}

struct Tool {
    name: &'static str,
    probe: &'static [&'static str],
    need: Need,
    install: Install,
    purpose: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool {
        name: "node",
        probe: &["node", "--version"],
        need: Need::Required,
        install: Install::Hint(
            "Node 20+ — nodejs.org or `winget install OpenJS.NodeJS.LTS` / `brew install node`",
        ),
        purpose: "desktop UI build, svelte-check, vitest, prettier",
    },
    Tool {
        name: "cargo-nextest",
        probe: &["cargo", "nextest", "--version"],
        need: Need::Required,
        install: Install::Binstall("cargo-nextest"),
        purpose: "test runner: parallel, per-test isolation, readable failures",
    },
    Tool {
        name: "cargo-deny",
        probe: &["cargo", "deny", "--version"],
        need: Need::Required,
        install: Install::Binstall("cargo-deny"),
        purpose: "licenses (no GPL by accident), advisories, sources",
    },
    Tool {
        name: "cargo-insta",
        probe: &["cargo", "insta", "--version"],
        need: Need::Required,
        install: Install::Binstall("cargo-insta"),
        purpose: "review snapshot changes: `cargo insta review`",
    },
    Tool {
        name: "cargo-llvm-cov",
        probe: &["cargo", "llvm-cov", "--version"],
        need: Need::Optional,
        install: Install::Binstall("cargo-llvm-cov"),
        purpose: "coverage: `cargo xtask coverage`",
    },
    Tool {
        name: "bacon",
        probe: &["bacon", "--version"],
        need: Need::Optional,
        install: Install::Binstall("bacon"),
        purpose: "background clippy/test on save (`bacon`, `bacon test`)",
    },
    Tool {
        name: "docker",
        probe: &["docker", "--version"],
        need: Need::From("S1"),
        install: Install::Hint("Docker Desktop"),
        purpose: "local LiveKit server",
    },
    Tool {
        name: "lk",
        probe: &["lk", "--version"],
        need: Need::From("S1"),
        install: Install::Hint("`winget install LiveKit.LiveKitCLI` / `brew install livekit-cli`"),
        purpose: "LiveKit CLI: tokens, rooms, `lk load-test`",
    },
    Tool {
        name: "gh",
        probe: &["gh", "--version"],
        need: Need::Optional,
        install: Install::Hint("`winget install GitHub.cli` / `brew install gh`"),
        purpose: "GitHub remote, PRs, CI logs — once the repo is on GitHub",
    },
];

/// `npm` and friends are `.cmd` shims on Windows; `Command` does not resolve those itself.
pub fn exe(name: &str) -> String {
    if cfg!(windows) && matches!(name, "npm" | "npx") {
        format!("{name}.cmd")
    } else {
        name.to_string()
    }
}

fn version(probe: &[&str]) -> Option<String> {
    let out = Command::new(exe(probe[0]))
        .args(&probe[1..])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

/// Fails with a setup hint if one of the named tools is missing.
pub fn require(names: &[&str]) -> Result {
    let missing: Vec<&str> = TOOLS
        .iter()
        .filter(|t| names.contains(&t.name) && version(t.probe).is_none())
        .map(|t| t.name)
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "missing tools: {} — run `cargo xtask setup`",
            missing.join(", ")
        ))
    }
}

pub fn doctor(root: &Path) -> Result {
    let mut required_missing = 0;
    println!("{:<15} {:<9} {:<34} purpose", "tool", "need", "status");
    for t in TOOLS {
        let need = match t.need {
            Need::Required => "required".to_string(),
            Need::From(s) => format!("from {s}"),
            Need::Optional => "optional".to_string(),
        };
        let status = match version(t.probe) {
            Some(v) => format!("✔ {}", truncate(&v, 32)),
            None => {
                if t.need == Need::Required {
                    required_missing += 1;
                }
                "✘ missing".to_string()
            }
        };
        println!("{:<15} {:<9} {:<34} {}", t.name, need, status, t.purpose);
        if status.starts_with('✘') {
            match t.install {
                Install::Binstall(c) => {
                    println!("{:<15} → cargo xtask setup  (cargo binstall {c})", "")
                }
                Install::Hint(h) => println!("{:<15} → {h}", ""),
            }
        }
    }

    let hooks = git_config(root, "core.hooksPath");
    let template = git_config(root, "commit.template");
    println!(
        "\ngit hooks      {}",
        if hooks.as_deref() == Some(".githooks") {
            "✔ .githooks (pre-commit: check --fast, pre-push: check)"
        } else {
            "✘ not active → cargo xtask setup"
        }
    );
    println!(
        "commit template {}",
        if template.is_some() {
            "✔ .gitmessage"
        } else {
            "✘ → cargo xtask setup"
        }
    );
    println!(
        "node_modules   {}",
        if root.join("apps/desktop/node_modules").exists() {
            "✔"
        } else {
            "✘ → cargo xtask setup"
        }
    );

    if required_missing > 0 {
        Err(format!("{required_missing} required tool(s) missing"))
    } else {
        Ok(())
    }
}

pub fn setup(root: &Path) -> Result {
    let binstall_missing = TOOLS
        .iter()
        .any(|t| matches!(t.install, Install::Binstall(_)) && version(t.probe).is_none());
    if binstall_missing && version(&["cargo", "binstall", "--version"]).is_none() {
        println!("── installing cargo-binstall (compiles once, a few minutes)");
        crate::cargo(root, &["install", "cargo-binstall", "--locked"])?;
    }
    for t in TOOLS {
        if let Install::Binstall(krate) = t.install {
            if version(t.probe).is_none() {
                println!("── installing {krate}");
                crate::cargo(root, &["binstall", "-y", "--locked", krate])?;
            }
        }
    }
    crate::ensure_node_modules(root, false)?;
    println!("── git: hooks + commit template");
    git(root, &["config", "core.hooksPath", ".githooks"])?;
    git(root, &["config", "commit.template", ".gitmessage"])?;
    println!();
    doctor(root)
}

fn git(root: &Path, args: &[&str]) -> Result {
    crate::run(Command::new("git").current_dir(root).args(args))
}

fn git_config(root: &Path, key: &str) -> Option<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["config", "--get", key])
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!v.is_empty()).then_some(v)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}
