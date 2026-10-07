<script lang="ts">
  // One window (planning doc, "UI-Prinzipien"): channels left, who is speaking in the middle,
  // mic and PTT state at the bottom. Channels are placeholders until the control plane (S6).
  import { onMount } from 'svelte';
  import { engine } from './lib/engine';
  import logo from './assets/logo.png';
  import type { TransmitMode } from './lib/types/TransmitMode';

  const channels = [
    { id: 'lobby', name: 'Lobby' },
    { id: 'zocken', name: 'Zocken' },
    { id: 'afk', name: 'AFK' },
  ];

  const modes: { id: TransmitMode; label: string }[] = [
    { id: 'voiceActivation', label: 'Sprachaktivierung' },
    { id: 'pushToTalk', label: 'Push-to-Talk' },
    { id: 'alwaysOn', label: 'Immer an' },
  ];

  let active = $state('lobby');
  let muted = $state(false);
  let deafened = $state(false);
  let mode = $state<TransmitMode>('voiceActivation');
  let theme = $state('dark');
  let protocol = $state<number | null>(null);
  // Controls appear only after the settings arrived, so a late load cannot overwrite a click.
  let loaded = $state(false);

  onMount(async () => {
    const info = await engine.info();
    if (info.type === 'ready') protocol = info.protocolVersion;
    const settings = await engine.settings();
    mode = settings.transmitMode;
    theme = settings.theme;
    loaded = true;
  });

  $effect(() => {
    document.documentElement.dataset.theme = theme;
  });

  function toggleMute() {
    muted = !muted;
    engine.send({ type: 'setMuted', muted });
  }

  function toggleDeafen() {
    deafened = !deafened;
    engine.send({ type: 'setDeafened', deafened });
  }

  function setMode(next: TransmitMode) {
    mode = next;
    engine.send({ type: 'setTransmitMode', mode: next });
  }

  const activeName = $derived(channels.find((c) => c.id === active)?.name ?? '');
</script>

<div class="shell">
  <nav class="channels" aria-label="Channels">
    <header><img src={logo} alt="" width="28" height="28" />yAPPA</header>
    {#each channels as channel (channel.id)}
      <button
        class="channel"
        aria-current={channel.id === active ? 'true' : undefined}
        onclick={() => (active = channel.id)}
      >
        <span aria-hidden="true">#</span>
        {channel.name}
      </button>
    {/each}
  </nav>

  <main class="stage">
    <h1>{activeName}</h1>
    <p class="empty">Noch niemand da. Voice kommt mit Spike S1.</p>
  </main>

  <footer class="bar">
    {#if loaded}
      <div class="group" role="group" aria-label="Senden">
        {#each modes as m (m.id)}
          <button aria-pressed={mode === m.id} onclick={() => setMode(m.id)}>{m.label}</button>
        {/each}
      </div>
      <div class="group">
        <button aria-pressed={muted} onclick={toggleMute}>{muted ? 'Mikro aus' : 'Mikro an'}</button
        >
        <button aria-pressed={deafened} onclick={toggleDeafen}>
          {deafened ? 'Taub' : 'Hören'}
        </button>
        <button onclick={() => (theme = theme === 'dark' ? 'light' : 'dark')}>Theme</button>
      </div>
    {/if}
    <span class="meta">
      {protocol === null ? '…' : protocol === 0 ? 'Browser-Modus' : `Protokoll v${protocol}`}
    </span>
  </footer>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) 1fr;
    grid-template-rows: 1fr auto;
    height: 100%;
  }

  .channels {
    grid-row: 1 / 3;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3);
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
  }

  .channels header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--font-size-lg);
    font-weight: 700;
    padding: var(--space-2) var(--space-2) var(--space-4);
  }

  .channels header img {
    border-radius: var(--radius-md);
  }

  .channel {
    display: flex;
    gap: var(--space-2);
    text-align: left;
    background: transparent;
    border-color: transparent;
    color: var(--color-text-muted);
  }

  .channel[aria-current='true'] {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }

  .stage {
    padding: var(--space-6);
    overflow: auto;
  }

  .stage h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  .empty {
    color: var(--color-text-muted);
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--color-surface);
    border-top: 1px solid var(--color-border);
  }

  .group {
    display: flex;
    gap: var(--space-2);
  }

  .meta {
    margin-left: auto;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
  }
</style>
