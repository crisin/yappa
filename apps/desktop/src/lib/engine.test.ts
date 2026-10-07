import { describe, expect, it } from 'vitest';
import { engine } from './engine';

// Outside Tauri the mock answers; these tests pin that browser mode works with real defaults.
describe('engine (browser mode)', () => {
  it('reports protocol 0 so the UI can show "browser mode"', async () => {
    expect(await engine.info()).toEqual({ type: 'ready', protocolVersion: 0 });
  });

  it('serves the Rust defaults from engine-protocol', async () => {
    const settings = await engine.settings();
    expect(settings.version).toBe(1);
    expect(settings.transmitMode).toBe('voiceActivation');
    expect(settings.denoiser).toBe('deepFilter');
    expect(settings.voiceBitrateKbps).toBeGreaterThanOrEqual(64);
  });

  it('hands out copies, not the shared defaults', async () => {
    const a = await engine.settings();
    a.theme = 'light';
    expect((await engine.settings()).theme).toBe('dark');
  });
});
