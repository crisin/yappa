// Everything the UI knows about the engine, in one reactive object: events go in through
// `apply`, the views read the fields, user actions become commands.
import type { Engine } from './engine';
import type { AudioDevice } from './types/AudioDevice';
import type { ConnectionState } from './types/ConnectionState';
import type { Event } from './types/Event';
import type { LogEntry } from './types/LogEntry';
import type { Participant } from './types/Participant';
import type { PeerLevel } from './types/PeerLevel';
import type { Settings } from './types/Settings';
import type { Stats } from './types/Stats';

const INVITE_KEY = 'yappa.invite';
/** Entries kept for the timeline in the debug panel. */
export const LOG_LIMIT = 300;
/** Floor of the level meters, dBFS. */
export const METER_FLOOR_DB = -60;

/** 0..1 for a meter bar. */
export function meterFill(db: number): number {
  return Math.min(1, Math.max(0, 1 - db / METER_FLOOR_DB));
}

const inviteProblems: [RegExp, string][] = [
  [/not a yAPPA invite/, 'Das ist keine yAPPA-Einladung. Sie beginnt mit „yappa1.“'],
  [/damaged|incompletely/, 'Die Einladung ist unvollständig – bitte noch einmal ganz kopieren.'],
  [/server address/, 'Die Einladung enthält keine gültige Serveradresse.'],
];

export class Session {
  connection = $state<ConnectionState>('disconnected');
  room = $state<string | null>(null);
  identity = $state<string | null>(null);
  participants = $state<Participant[]>([]);
  inputDb = $state(-120);
  /** False while our microphone is muted. */
  sending = $state(true);
  peerLevels = $state<Record<string, PeerLevel>>({});
  stats = $state<Stats | null>(null);
  log = $state<LogEntry[]>([]);
  inputs = $state<AudioDevice[]>([]);
  outputs = $state<AudioDevice[]>([]);
  settings = $state<Settings | null>(null);
  muted = $state(false);
  deafened = $state(false);
  /** Per person, as set here: volume in dB and local mute. */
  peerGainDb = $state<Record<string, number>>({});
  peerMuted = $state<Record<string, boolean>>({});
  /** The last problem the engine reported, until dismissed. */
  error = $state<string | null>(null);
  invite = $state('');
  inviteProblem = $state<string | null>(null);

  readonly #engine: Engine;

  constructor(engine: Engine) {
    this.#engine = engine;
    try {
      this.invite = localStorage.getItem(INVITE_KEY) ?? '';
    } catch {
      // No storage (private mode): the invite is simply not remembered.
    }
  }

  /** Starts listening. Returns the function that stops it. */
  async start(): Promise<() => void> {
    const stop = await this.#engine.subscribe((event) => this.apply(event));
    this.settings = await this.#engine.settings();
    const earlier = await this.#engine.recentLogs();
    this.log = [...earlier, ...this.log].slice(-LOG_LIMIT);
    return stop;
  }

  apply(event: Event) {
    switch (event.type) {
      case 'connection':
        this.connection = event.state;
        if (event.state === 'disconnected') {
          this.room = null;
          this.identity = null;
          this.peerLevels = {};
        }
        break;
      case 'joined':
        this.room = event.room;
        this.identity = event.identity;
        this.error = null;
        break;
      case 'participants':
        this.participants = event.participants;
        break;
      case 'levels':
        this.inputDb = event.inputDb;
        this.sending = event.gateOpen;
        this.peerLevels = Object.fromEntries(event.peers.map((peer) => [peer.identity, peer]));
        break;
      case 'stats':
        this.stats = event.stats;
        break;
      case 'devices':
        this.inputs = event.inputs;
        this.outputs = event.outputs;
        break;
      case 'log':
        this.log = [...this.log, event.entry].slice(-LOG_LIMIT);
        break;
      case 'error':
        this.error = event.message;
        break;
      case 'ready':
        break;
    }
  }

  get inRoom(): boolean {
    return this.connection !== 'disconnected';
  }

  async join() {
    this.inviteProblem = null;
    let invite;
    try {
      invite = await this.#engine.decodeInvite(this.invite);
    } catch (reason) {
      const text = String(reason);
      this.inviteProblem =
        inviteProblems.find(([pattern]) => pattern.test(text))?.[1] ??
        'Die Einladung ist ungültig.';
      return;
    }
    try {
      localStorage.setItem(INVITE_KEY, this.invite.trim());
    } catch {
      // see constructor
    }
    this.error = null;
    await this.#engine.send({ type: 'join', url: invite.url, token: invite.token });
  }

  leave() {
    return this.#engine.send({ type: 'leave' });
  }

  toggleMute() {
    this.muted = !this.muted;
    return this.#engine.send({ type: 'setMuted', muted: this.muted });
  }

  toggleDeafen() {
    this.deafened = !this.deafened;
    return this.#engine.send({ type: 'setDeafened', deafened: this.deafened });
  }

  /** `null` = system default. */
  setInput(id: string | null) {
    if (this.settings) this.settings.inputDevice = id;
    return this.#engine.send({ type: 'setInputDevice', id });
  }

  setOutput(id: string | null) {
    if (this.settings) this.settings.outputDevice = id;
    return this.#engine.send({ type: 'setOutputDevice', id });
  }

  refreshDevices() {
    return this.#engine.send({ type: 'refreshDevices' });
  }

  setPeerGain(identity: string, gainDb: number) {
    this.peerGainDb[identity] = gainDb;
    return this.#engine.send({ type: 'setPeerVolume', identity, track: 'voice', gainDb });
  }

  togglePeerMute(identity: string) {
    const muted = !this.peerMuted[identity];
    this.peerMuted[identity] = muted;
    return this.#engine.send({ type: 'setPeerMuted', identity, track: 'voice', muted });
  }

  exportLogs(): Promise<string> {
    return this.#engine.exportLogs();
  }
}
