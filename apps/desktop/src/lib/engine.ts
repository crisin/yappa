// The UI's only door to the engine. Speaks engine-protocol (generated types), nothing else.
// Outside Tauri (plain `npm run dev` in a browser) a mock answers, so the UI stays workable.
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Command } from './types/Command';
import type { Event } from './types/Event';
import type { Settings } from './types/Settings';
import defaultSettings from './types/defaultSettings.json';

// Generated from `Settings::default()` in engine-protocol — the mock cannot drift from Rust.
const defaults = defaultSettings as Settings;

export const engine = {
  info(): Promise<Event> {
    return isTauri()
      ? invoke<Event>('engine_info')
      : Promise.resolve({ type: 'ready', protocolVersion: 0 });
  },
  settings(): Promise<Settings> {
    return isTauri()
      ? invoke<Settings>('get_settings')
      : Promise.resolve(structuredClone(defaults));
  },
  send(command: Command): Promise<void> {
    return isTauri() ? invoke<void>('engine_command', { command }) : Promise.resolve();
  },
};
