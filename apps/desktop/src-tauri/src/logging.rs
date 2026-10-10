//! Logging, set up once at start. Three destinations for the same events:
//!
//! - **file** — JSON lines in the app's log folder, one file per day, seven days kept.
//!   Everything from our crates at debug level (including one stats line per second while
//!   in a call), other crates from info. This is what gets exported and analysed.
//! - **UI** — info and above as [`LogEntry`], for the timeline in the debug panel. The last
//!   entries are kept so a panel opened later still shows what led up to now.
//! - **stderr** — the same, readable, for `cargo xtask dev`.
//!
//! Tokens and invites are never logged; server addresses are.

use engine_protocol::{LogEntry, LogLevel};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

const FILE_PREFIX: &str = "yappa";
const FILE_SUFFIX: &str = "log";
const DAYS_KEPT: usize = 7;
const RECENT: usize = 300;
/// Our own crates: these log at debug level into the file.
const OWN: [&str; 4] = ["audio_engine", "transport", "yappa_desktop_lib", "stats"];

type Listener = Box<dyn Fn(LogEntry) + Send + Sync>;

/// Keeps the file writer alive and gives access to what the UI layer collected.
pub struct Logging {
    pub dir: PathBuf,
    timeline: Arc<Timeline>,
    _file: tracing_appender::non_blocking::WorkerGuard,
}

#[derive(Default)]
struct Timeline {
    recent: Mutex<VecDeque<LogEntry>>,
    listener: Mutex<Option<Listener>>,
}

impl Logging {
    /// The newest entries, oldest first.
    pub fn recent(&self) -> Vec<LogEntry> {
        self.timeline
            .recent
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect()
    }

    /// From now on every new entry also goes to `listener`.
    pub fn on_entry(&self, listener: impl Fn(LogEntry) + Send + Sync + 'static) {
        *self.timeline.listener.lock().unwrap() = Some(Box::new(listener));
    }

    /// Flushes what is still queued for the file. Call before reading the files.
    pub fn log_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(FILE_PREFIX))
            })
            .collect();
        files.sort();
        files
    }
}

/// Call once. Fails only if the log folder cannot be created or logging is already set up.
pub fn init(dir: &Path) -> Result<Logging, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(DAYS_KEPT)
        .build(dir)
        .map_err(|e| e.to_string())?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let own_at = |level: Level| {
        OWN.iter()
            .fold(Targets::new().with_default(Level::INFO), |targets, name| {
                targets.with_target(*name, level)
            })
    };
    let file = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_writer(writer)
        .with_filter(own_at(Level::DEBUG));
    let stderr = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(std::io::stderr)
        .with_filter(own_at(Level::INFO));
    let timeline = Arc::new(Timeline::default());
    let ui = TimelineLayer(timeline.clone()).with_filter(own_at(Level::INFO));

    tracing_subscriber::registry()
        .with(file)
        .with(stderr)
        .with(ui)
        .try_init()
        .map_err(|e| e.to_string())?;

    // A panic on any thread ends up in the file, not only on a console nobody sees.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "panic");
        default_hook(info);
    }));

    Ok(Logging {
        dir: dir.to_path_buf(),
        timeline,
        _file: guard,
    })
}

struct TimelineLayer(Arc<Timeline>);

impl<S: Subscriber> Layer<S> for TimelineLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        let level = match *metadata.level() {
            Level::ERROR => LogLevel::Error,
            Level::WARN => LogLevel::Warn,
            _ => LogLevel::Info,
        };
        let mut text = Text::default();
        event.record(&mut text);
        let entry = LogEntry {
            time_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            level,
            target: metadata.target().to_string(),
            message: text.finish(),
        };
        {
            let mut recent = self.0.recent.lock().unwrap();
            if recent.len() == RECENT {
                recent.pop_front();
            }
            recent.push_back(entry.clone());
        }
        if let Some(listener) = self.0.listener.lock().unwrap().as_ref() {
            listener(entry);
        }
    }
}

/// The message first, then the fields as `key=value`.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Text {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields,
            (false, false) => format!("{} ({})", self.message, self.fields),
        }
    }
}

impl Visit for Text {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let separator = if self.fields.is_empty() { "" } else { ", " };
            let _ = write!(self.fields, "{separator}{}={value:?}", field.name());
        }
    }
}
