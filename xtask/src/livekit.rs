//! `cargo xtask livekit` — the local LiveKit server (stage 0, dev keys) and the network
//! impairments spike S1 needs: packet loss and a "pulled cable".
//!
//!   up (default)     start server + netem sidecar, detached
//!   down             stop and remove both
//!   logs             follow the server log
//!   loss <percent>   drop that share of packets in both directions; 0 clears
//!   cut <seconds>    drop everything for that long, then restore (cable pull)

use crate::Result;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const COMPOSE: &str = "infra/livekit/dev.yml";
const IFACE: &str = "eth0";

pub fn run(root: &Path, args: &[String]) -> Result {
    crate::tools::require(&["docker"])?;
    let arg = |i: usize| args.get(i).map(String::as_str);
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
            "unknown: livekit {other} — up | down | logs | loss <percent> | cut <seconds>"
        )),
    }
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
