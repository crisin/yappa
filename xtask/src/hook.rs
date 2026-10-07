//! Claude Code hook entry points (wired in `.claude/settings.json`). Rust instead of shell,
//! so the same hook works on Windows and macOS.
//!
//!   post-edit      PostToolUse on Edit|Write: format the touched file (rustfmt / prettier).
//!                  Never blocks — a file mid-refactor may not parse yet.
//!   session-start  SessionStart: prints branch, the "Now"/"Next" tasks and the gate command;
//!                  stdout becomes context for the session.

use crate::Result;
use std::io::Read;
use std::path::Path;
use std::process::Command;

/// Prettier handles these inside apps/desktop.
const PRETTIER_EXT: &[&str] = &["svelte", "ts", "js", "css", "json", "html"];

pub fn run(root: &Path, name: Option<&str>) -> Result {
    match name {
        Some("post-edit") => {
            post_edit(root);
            Ok(())
        }
        Some("session-start") => session_start(root),
        other => Err(format!("unknown hook {other:?}")),
    }
}

fn post_edit(root: &Path) {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&input) else {
        return;
    };
    let Some(file) = json["tool_input"]["file_path"].as_str() else {
        return;
    };
    let path = Path::new(file);
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if crate::GENERATED.iter().any(|g| rel_str.starts_with(g)) || !path.exists() {
        return;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if ext == "rs" {
        // Edition and width come from rustfmt.toml (rustfmt alone does not read Cargo.toml).
        let _ = Command::new("rustfmt").current_dir(root).arg(path).output();
    } else if rel_str.starts_with("apps/desktop/") && PRETTIER_EXT.contains(&ext) {
        let bin = root
            .join("apps/desktop/node_modules/.bin")
            .join(if cfg!(windows) {
                "prettier.cmd"
            } else {
                "prettier"
            });
        if bin.exists() {
            let _ = Command::new(bin)
                .current_dir(root.join("apps/desktop"))
                .args(["--write", "--log-level", "silent"])
                .arg(path)
                .output();
        }
    }
}

fn session_start(root: &Path) -> Result {
    let branch = Command::new("git")
        .current_dir(root)
        .args(["branch", "--show-current"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let tasks = std::fs::read_to_string(root.join("TASKS.md")).unwrap_or_default();

    println!("yAPPA variant B · branch `{branch}`");
    for section in ["## Now", "## Next"] {
        if let Some(body) = section_body(&tasks, section) {
            println!("{section} (TASKS.md)");
            // Only the task lines (wrapped lines joined), at most 4, to keep the context small.
            for task in tasks_in(body).iter().take(4) {
                println!("{task}");
            }
        }
    }
    println!(
        "Gate: `cargo xtask check` (fast: `--fast`). Work on a `task/<name>` branch, never on main."
    );
    Ok(())
}

fn tasks_in(body: &str) -> Vec<String> {
    let mut tasks: Vec<String> = Vec::new();
    for line in body.lines() {
        if line.starts_with("- [") {
            tasks.push(line.trim_end().to_string());
        } else if let (Some(last), true) = (tasks.last_mut(), line.starts_with("  ")) {
            last.push(' ');
            last.push_str(line.trim());
        }
    }
    tasks
}

fn section_body<'a>(text: &'a str, heading: &str) -> Option<&'a str> {
    let start = text.find(heading)? + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    Some(&rest[..end])
}
