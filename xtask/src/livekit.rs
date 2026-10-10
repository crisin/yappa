//! `cargo xtask livekit` — the local LiveKit server (stage 0, dev keys) and the network
//! impairments spike S1 needs: packet loss and a "pulled cable".
//!
//!   up (default)     start server + netem sidecar, detached
//!   down             stop and remove both
//!   logs             follow the server log
//!   loss <percent>   drop that share of packets in both directions; 0 clears
//!   cut <seconds>    drop everything for that long, then restore (cable pull)
//!   token <name> [--room gang] [--days 30] [--jwt]
//!                    an invite for one person (server address + join token as one line
//!                    to paste into the client), signed with the keys in
//!                    infra/livekit/.env (dev keys if there is no .env) — the stand-in
//!                    for the control plane until S6. --jwt prints the bare token.

use crate::Result;
use livekit_api::access_token::{AccessToken, VideoGrants};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const COMPOSE: &str = "infra/livekit/dev.yml";
const ENV_FILE: &str = "infra/livekit/.env";
const IFACE: &str = "eth0";

pub fn run(root: &Path, args: &[String]) -> Result {
    let arg = |i: usize| args.get(i).map(String::as_str);
    if arg(0) == Some("token") {
        return token(root, &args[1..]);
    }
    crate::tools::require(&["docker"])?;
    match arg(0) {
        None | Some("up") => {
            compose(root, &["up", "-d", "--wait"])?;
            println!("LiveKit: ws://localhost:7880 (devkey / secret), media on udp/7882");
            Ok(())
        }
        Some("down") => compose(root, &["down"]),
        Some("logs") => compose(root, &["logs", "-f", "livekit"]),
        Some("loss") => {
            let percent = parse(arg(1), "loss <percent 0-100>")?;
            if percent > 100 {
                return Err("loss <percent 0-100>".into());
            }
            loss(root, percent)?;
            println!("packet loss: {percent} % in both directions");
            Ok(())
        }
        Some("cut") => {
            let seconds = parse(arg(1), "cut <seconds>")?;
            loss(root, 100)?;
            println!("link cut for {seconds} s …");
            std::thread::sleep(Duration::from_secs(seconds.into()));
            loss(root, 0)?;
            println!("link restored");
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown: livekit {other} — up | down | logs | loss <percent> | cut <seconds> | token <name>"
        )),
    }
}

/// Prints an invite (or the bare token) for one person. Whoever holds the token can join that room
/// until it expires; the only way to revoke it is a new key pair on the server.
fn token(root: &Path, args: &[String]) -> Result {
    const USAGE: &str = "usage: cargo xtask livekit token <name> [--room gang] [--days 30] [--jwt]";
    let name = args.first().filter(|a| !a.starts_with("--")).ok_or(USAGE)?;
    let option = |flag: &str| {
        let i = args.iter().position(|a| a == flag)?;
        args.get(i + 1).map(String::as_str)
    };
    let room = option("--room").unwrap_or("gang");
    let days: u64 = option("--days")
        .unwrap_or("30")
        .parse()
        .map_err(|_| USAGE)?;

    let env = read_env(&root.join(ENV_FILE));
    let get = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    let (key, secret, url) = match (get("LIVEKIT_API_KEY"), get("LIVEKIT_API_SECRET")) {
        (Some(key), Some(secret)) => {
            let domain =
                get("YAPPA_DOMAIN").ok_or(format!("YAPPA_DOMAIN missing in {ENV_FILE}"))?;
            (key, secret, format!("wss://{domain}"))
        }
        _ => {
            eprintln!("no keys in {ENV_FILE} — signing with the dev keys for the local server");
            ("devkey", "secret", "ws://localhost:7880".to_string())
        }
    };
    let jwt = AccessToken::with_api_key(key, secret)
        .with_identity(name)
        .with_name(name)
        .with_ttl(Duration::from_secs(days * 24 * 3600))
        .with_grants(VideoGrants {
            room_join: true,
            room: room.to_string(),
            ..Default::default()
        })
        .to_jwt()
        .map_err(|e| format!("token: {e}"))?;

    eprintln!("{name} · room '{room}' · valid {days} days · {url}");
    if args.iter().any(|a| a == "--jwt") {
        eprintln!("bare token, for: yappa-poc join --url {url} --token <token>");
        println!("{jwt}");
    } else {
        eprintln!("invite — paste it into the client under \"Einladung\":");
        println!("{}", api_types::Invite { url, token: jwt }.encode());
    }
    Ok(())
}

/// KEY=VALUE lines; empty values count as not set.
fn read_env(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').to_string()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

fn parse(arg: Option<&str>, usage: &str) -> Result<u32> {
    arg.and_then(|a| a.parse().ok())
        .ok_or_else(|| format!("usage: cargo xtask livekit {usage}"))
}

/// Egress (server → clients) through netem, ingress (clients → server) through a random
/// drop filter — together a lossy link in both directions, signalling included.
fn loss(root: &Path, percent: u32) -> Result {
    // Clearing fails when nothing is set; that is fine.
    let _ = tc(root, &["qdisc", "delete", "dev", IFACE, "root"], true);
    let _ = tc(root, &["qdisc", "delete", "dev", IFACE, "ingress"], true);
    if percent == 0 {
        return Ok(());
    }
    let egress = format!("{percent}%");
    tc(
        root,
        &[
            "qdisc", "replace", "dev", IFACE, "root", "netem", "loss", &egress,
        ],
        false,
    )?;
    tc(
        root,
        &["qdisc", "add", "dev", IFACE, "handle", "ffff:", "ingress"],
        false,
    )?;
    let filter = [
        "filter", "add", "dev", IFACE, "parent", "ffff:", "protocol", "ip", "u32", "match", "u32",
        "0", "0", "action",
    ];
    // gact drops one packet in N; 100 % is a plain drop.
    let one_in = (100.0 / f64::from(percent)).round().to_string();
    let action: &[&str] = if percent == 100 {
        &["drop"]
    } else {
        &["gact", "pass", "random", "netrand", "drop", &one_in]
    };
    tc(root, &[&filter[..], action].concat(), false)
}

fn tc(root: &Path, args: &[&str], quiet: bool) -> Result {
    let mut cmd = docker(root);
    cmd.args(["exec", "-T", "netem", "tc"]).args(args);
    if quiet {
        let out = cmd.output().map_err(|e| format!("docker: {e}"))?;
        return if out.status.success() {
            Ok(())
        } else {
            Err("tc failed".into())
        };
    }
    crate::run(&mut cmd)
}

fn compose(root: &Path, args: &[&str]) -> Result {
    crate::run(docker(root).args(args))
}

fn docker(root: &Path) -> Command {
    let mut cmd = Command::new("docker");
    cmd.current_dir(root).args(["compose", "-f", COMPOSE]);
    cmd
}
