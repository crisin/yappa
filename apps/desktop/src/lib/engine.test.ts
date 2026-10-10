import { describe, expect, it } from 'vitest';
import { engine } from './engine';
import { createMockEngine, encodeInvite } from './mockEngine';
import type { Event } from './types/Event';

// Outside Tauri the mock answers; these tests pin that browser mode works with real defaults
// and behaves like the engine as far as the UI can tell.
describe('engine (browser mode)', () => {
  it('reports protocol 0 so the UI can show "browser mode"', async () => {
    expect((await engine.info()).protocolVersion).toBe(0);
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

  it('decodes what the invite format encodes, also wrapped by a chat window', async () => {
    const invite = { url: 'wss://voice.example', token: 'a.b-c_d' };
    const text = encodeInvite(invite);
    expect(text.startsWith('yappa1.')).toBe(true);
    expect(await engine.decodeInvite(` ${text.slice(0, 12)}\n${text.slice(12)} `)).toEqual(invite);
  });

  it('rejects other text and damaged invites with a reason', async () => {
    await expect(engine.decodeInvite('hallo')).rejects.toMatch(/not a yAPPA invite/);
    await expect(engine.decodeInvite('yappa1.%%%')).rejects.toMatch(/damaged/);
  });

  it('walks through a join like the real engine: connecting, joined, people, connected', async () => {
    const mock = createMockEngine();
    const seen: Event[] = [];
    const stop = await mock.subscribe((event) => seen.push(event));
    await mock.send({ type: 'join', url: 'wss://voice.example', token: 't' });
    await mock.send({ type: 'leave' });
    stop();
    const kinds = seen.filter((e) => e.type !== 'log').map((e) => e.type);
    expect(kinds).toEqual([
      'connection',
      'joined',
      'participants',
      'connection',
      'participants',
      'connection',
    ]);
    expect(seen.at(-2)).toEqual({ type: 'connection', state: 'disconnected' });
  });
});
