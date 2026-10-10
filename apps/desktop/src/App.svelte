<script lang="ts">
  // One window (planning doc, "UI-Prinzipien"): devices and microphone test on the left,
  // the room in the middle, mic and connection controls at the bottom, and a debug panel
  // that can be shown on the right. Channels and login come with the control plane (S6);
  // until then one pasted invite leads into one room.
  import { onMount } from 'svelte';
  import { engine, type AppInfo } from './lib/engine';
  import { Session } from './lib/session.svelte';
  import DebugPanel from './lib/DebugPanel.svelte';
  import JoinCard from './lib/JoinCard.svelte';
  import Meter from './lib/Meter.svelte';
  import Participants from './lib/Participants.svelte';
  import logo from './assets/logo.png';

  const session = new Session(engine);
  let info = $state<AppInfo | null>(null);
  let theme = $state('dark');
  // The panel stays the way the tester left it.
  const DEBUG_KEY = 'yappa.debug';
  let debug = $state(remembered());

  function remembered(): boolean {
    try {
      return localStorage.getItem(DEBUG_KEY) === '1';
    } catch {
      return false;
    }
  }

  function toggleDebug() {
    debug = !debug;
    try {
      localStorage.setItem(DEBUG_KEY, debug ? '1' : '0');
    } catch {
      // No storage: the choice lasts until the window closes.
    }
  }

  onMount(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void (async () => {
      info = await engine.info();
      if (info.debugPanel) debug = true;
      stop = await session.start();
      if (gone) stop();
      theme = session.settings?.theme ?? 'dark';
    })();
    return () => {
      gone = true;
      stop?.();
    };
  });

  $effect(() => {
    document.documentElement.dataset.theme = theme;
  });

  const status = $derived(
    {
      disconnected: 'Nicht verbunden',
      connecting: 'Verbinde …',
      connected: 'Verbunden',
      reconnecting: 'Verbindung verloren – verbinde neu …',
    }[session.connection],
  );

  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);
</script>

<div class="shell" class:with-debug={debug}>
  <nav class="side" aria-label="Geräte">
    <header><img src={logo} alt="" width="28" height="28" />yAPPA</header>

    <label for="input">Mikrofon</label>
    <select
      id="input"
      value={deviceValue(session.settings?.inputDevice)}
      onchange={(e) => session.setInput(deviceId(e.currentTarget.value))}
    >
      <option value="">Standardgerät</option>
      {#each session.inputs as device (device.id)}
        <option value={device.id}>{device.name}</option>
      {/each}
    </select>
    <Meter db={session.inputDb} active={session.sending} label="Mikrofonpegel" />
    <p class="note">
      {session.sending ? 'Sprich – der Balken zeigt, was ankommt.' : 'Mikrofon ist aus.'}
    </p>

    <label for="output">Ausgabe</label>
    <select
      id="output"
      value={deviceValue(session.settings?.outputDevice)}
      onchange={(e) => session.setOutput(deviceId(e.currentTarget.value))}
    >
      <option value="">Standardgerät</option>
      {#each session.outputs as device (device.id)}
        <option value={device.id}>{device.name}</option>
      {/each}
    </select>

    <button class="quiet" onclick={() => session.refreshDevices()}>Geräte neu einlesen</button>
  </nav>

  <main class="stage">
    {#if session.error}
      <div class="error" role="alert">
        <span>{session.error}</span>
        <button class="quiet" onclick={() => (session.error = null)}>Schließen</button>
      </div>
    {/if}

    {#if session.inRoom && session.connection !== 'connecting'}
      <h1>{session.room ?? 'Raum'}</h1>
      <Participants {session} />
    {:else}
      <JoinCard {session} />
    {/if}
  </main>

  {#if debug}
    <DebugPanel {session} {info} />
  {/if}

  <footer class="bar">
    <span class="status" class:warn={session.connection === 'reconnecting'} role="status">
      <span class="dot" class:on={session.connection === 'connected'}></span>
      {status}
    </span>
    <div class="group">
      <button aria-pressed={session.muted} onclick={() => session.toggleMute()}>
        {session.muted ? 'Mikro aus' : 'Mikro an'}
      </button>
      <button aria-pressed={session.deafened} onclick={() => session.toggleDeafen()}>
        {session.deafened ? 'Taub' : 'Hören'}
      </button>
      {#if session.inRoom}
        <button class="leave" onclick={() => session.leave()}>Verlassen</button>
      {/if}
    </div>
    <div class="group end">
      <button aria-pressed={debug} onclick={toggleDebug}>Debug</button>
      <button onclick={() => (theme = theme === 'dark' ? 'light' : 'dark')}>Theme</button>
      <span class="meta">
        {info === null ? '…' : info.protocolVersion === 0 ? 'Browser-Modus' : `v${info.version}`}
      </span>
    </div>
  </footer>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) 1fr;
    grid-template-rows: 1fr auto;
    height: 100%;
  }

  .shell.with-debug {
    grid-template-columns: var(--sidebar-width) 1fr auto;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
    overflow-y: auto;
  }

  .side header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--font-size-lg);
    font-weight: 700;
    padding: var(--space-2) var(--space-2) var(--space-4);
  }

  .side header img {
    border-radius: var(--radius-md);
  }

  .side label {
    margin-top: var(--space-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
    min-width: 0;
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .side .quiet {
    margin-top: var(--space-3);
  }

  .stage {
    padding: var(--space-6);
    overflow: auto;
    min-width: 0;
  }

  .stage h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  .error {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    user-select: text;
  }

  .bar {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--color-surface);
    border-top: 1px solid var(--color-border);
  }

  .status {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: calc(var(--sidebar-width) - var(--space-4));
    color: var(--color-text-muted);
  }

  .status.warn {
    color: var(--color-warning);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-text-muted);
  }

  .dot.on {
    background: var(--color-speaking);
  }

  .group {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .group.end {
    margin-left: auto;
  }

  .leave {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .meta {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
  }
</style>
