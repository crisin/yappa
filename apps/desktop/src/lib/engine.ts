// The UI's only door to the engine. Speaks engine-protocol (generated types), nothing else.
// Outside Tauri (plain `npm run dev` in a browser) a mock answers, so the UI stays workable.
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Command } from './types/Command';
import type { Event } from './types/Event';
import type { Settings } from './types/Settings';

const mockSettings: Settings = {
  version: 1,
  inputDevice: null,
  outputDevice: null,
  transmitMode: 'voiceActivation',
  vadThresholdDb: -50,
  vadHoldMs: 300,
  denoiser: 'deepFilter',
  highPassHz: 80,
  eq: [],
  compressor: {
    enabled: true,
    thresholdDb: -18,
    ratio: 3,
    attackMs: 5,
    releaseMs: 120,
    makeupDb: 3,
  },
  voiceBitrateKbps: 80,
  theme: 'dark',
};

export const engine = {
  info(): Promise<Event> {
    return isTauri()
      ? invoke<Event>('engine_info')
      : Promise.resolve({ type: 'ready', protocolVersion: 0 });
  },
  settings(): Promise<Settings> {
    return isTauri() ? invoke<Settings>('get_settings') : Promise.resolve(mockSettings);
  },
  send(command: Command): Promise<void> {
    return isTauri() ? invoke<void>('engine_command', { command }) : Promise.resolve();
  },
};
