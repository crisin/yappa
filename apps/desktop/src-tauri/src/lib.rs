//! Tauri shell. It owns windows and OS integration and forwards `engine-protocol` messages;
//! it never does audio itself. Until the engine exists (S1), commands are validated and
//! remembered here so the UI can be built against the real contract.

use engine_protocol::{Command, Event, Settings, PROTOCOL_VERSION};
use std::sync::Mutex;

#[derive(Default)]
struct EngineStub {
    settings: Mutex<Settings>,
}

#[tauri::command]
fn engine_info() -> Event {
    Event::Ready {
        protocol_version: PROTOCOL_VERSION,
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, EngineStub>) -> Settings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
fn engine_command(state: tauri::State<'_, EngineStub>, command: Command) -> Result<(), String> {
    match command {
        Command::ApplySettings { settings } => {
            settings.validate()?;
            *state.settings.lock().map_err(|e| e.to_string())? = settings;
        }
        other => eprintln!("engine stub: {other:?} (no engine before S1)"),
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(EngineStub::default())
        .invoke_handler(tauri::generate_handler![
            engine_info,
            get_settings,
            engine_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running yAPPA");
}
