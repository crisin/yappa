//! The audio engine.
//!
//! Commands go in, events come out ([`engine_protocol`]); the media plane is a
//! [`transport::Transport`], the sound hardware an [`AudioIo`]. This crate knows neither
//! Tauri nor LiveKit, so it runs the same inside the desktop shell, in a headless binary and
//! in tests with fakes on both sides.
//!
//! Threads:
//! - **control** ([`engine`]) — commands, transport events, levels and stats timers.
//! - **pump** — capture ring → 10 ms blocks → transport. Not real-time: the transport
//!   encodes on it.
//! - **device callbacks** ([`device`], [`mixer`]) — real-time: rings and atomics only.

pub mod device;
pub mod engine;
pub mod mixer;
pub mod stats;

pub use device::{AudioIo, CpalIo};
pub use engine::{spawn, EngineConfig, EngineHandle};
