//! The engine without a window: the same engine, transport and devices as the app, events
//! printed as JSON lines. For measuring, for debugging on a machine without a display, and
//! for checking the engine against a server before the UI is involved.
//!
//!   cargo run -p yappa-desktop --example headless -- --invite <yappa1.…> [--seconds 20]
//!   cargo run -p yappa-desktop --example headless -- --url ws://localhost:7880 --token <jwt>
//!
//! Levels are printed twice per second instead of thirty times. Logs go to stderr and to
//! `target/headless-logs/`.

use api_types::Invite;
use engine_protocol::{Command, Event, Settings};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        let i = args.iter().position(|a| a == name)?;
        args.get(i + 1).cloned()
    };
    let invite = match (value("--invite"), value("--url"), value("--token")) {
        (Some(text), _, _) => Invite::decode(&text).map_err(|e| e.to_string())?,
        (None, Some(url), Some(token)) => Invite { url, token },
        _ => return Err("usage: headless --invite <text> | --url <ws://…> --token <jwt>".into()),
    };
    let seconds: u64 = value("--seconds")
        .map(|s| s.parse().map_err(|_| "--seconds: not a number"))
        .transpose()?
        .unwrap_or(20);

    let _logging = yappa_desktop_lib::logging::init("target/headless-logs".as_ref())?;
    let levels = AtomicU32::new(0);
    let engine = yappa_desktop_lib::start_engine(Settings::default(), move |event| {
        if matches!(event, Event::Levels { .. }) && !levels.fetch_add(1, Relaxed).is_multiple_of(15)
        {
            return;
        }
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
    })
    .map_err(|e| e.to_string())?;

    engine.send(Command::Join {
        url: invite.url,
        token: invite.token,
    });
    std::thread::sleep(Duration::from_secs(seconds));
    engine.send(Command::Leave);
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
