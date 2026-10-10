<script lang="ts">
  // Everything a tester can read out when something sounds wrong: connection, network,
  // devices, and what the engine logged. The same numbers go into the log file once per
  // second, so "it crackled around nine" can be looked up afterwards.
  import type { AppInfo } from './engine';
  import type { Session } from './session.svelte';
  import type { DeviceStats } from './types/DeviceStats';

  let { session, info }: { session: Session; info: AppInfo | null } = $props();

  const connectionLabel = {
    disconnected: 'getrennt',
    connecting: 'verbinde',
    connected: 'verbunden',
    reconnecting: 'verbinde neu',
  } as const;

  type Row = { label: string; value: string; warn?: boolean };

  const fixed = (n: number, digits = 1) => n.toFixed(digits).replace('.', ',');
  const net = $derived(session.stats?.network ?? null);

  const network = $derived<Row[]>(
    net
      ? [
          {
            label: 'Laufzeit zum Server',
            value: `${fixed(net.rttMs, 0)} ms`,
            warn: net.rttMs > 150,
          },
          {
            label: 'Paketverlust Empfang',
            value: `${fixed(net.lossPercentRecent)} %`,
            warn: net.lossPercentRecent > 5,
          },
          {
            label: 'Paketverlust Senden',
            value: `${fixed(net.uplinkLossPercent)} %`,
            warn: net.uplinkLossPercent > 5,
          },
          {
            label: 'Ersetzte Samples',
            value: `${fixed(net.concealedPercentRecent)} % (gesamt ${fixed(net.concealedPercentTotal)} %)`,
            warn: net.concealedPercentRecent > 2,
          },
          { label: 'Jitter', value: `${fixed(net.jitterMs, 0)} ms`, warn: net.jitterMs > 30 },
          {
            label: 'Jitterbuffer',
            value: `${fixed(net.jitterBufferMs, 0)} ms`,
            warn: net.jitterBufferMs > 120,
          },
          { label: 'Senderate', value: `${fixed(net.sendKbpsRecent, 0)} kbit/s` },
          { label: 'Codec', value: net.sendCodec || '–' },
          {
            label: 'Pakete',
            value: `${net.packetsSent} gesendet · ${net.packetsReceived} empfangen · ${net.packetsLost} verloren`,
          },
        ]
      : [],
  );

  function deviceRows(device: DeviceStats | null, xrunLabel: string): Row[] {
    if (!device) return [{ label: 'Gerät', value: 'nicht geöffnet', warn: true }];
    const ms = fixed((device.callbackFrames / device.sampleRate) * 1000);
    return [
      { label: 'Gerät', value: device.name },
      { label: 'Format', value: `${device.sampleRate} Hz · ${device.channels} Kanäle` },
      { label: 'Puffer', value: `${device.callbackFrames} Frames (${ms} ms)` },
      { label: xrunLabel, value: String(device.xruns), warn: device.xruns > 0 },
    ];
  }

  const stats = $derived(session.stats);
  const pipeline = $derived<Row[]>(
    stats
      ? [
          {
            label: 'Längster Sende-Abstand',
            value: `${fixed(stats.sendIntervalMaxMs)} ms`,
            warn: stats.sendIntervalMaxMs > 30,
          },
          {
            label: 'Verworfene Sende-Blöcke',
            value: String(stats.sendBlocksDropped),
            warn: stats.sendBlocksDropped > 0,
          },
          {
            label: 'Verworfene Empfangs-Frames',
            value: String(stats.playoutFramesDropped),
            warn: stats.playoutFramesDropped > 0,
          },
          {
            label: 'Sprünge in der Wiedergabe',
            value: String(stats.playoutSkips),
            warn: stats.playoutSkips > 0,
          },
        ]
      : [],
  );

  const timeline = $derived(session.log.slice(-120).reverse());
  const clock = (ms: number) => new Date(ms).toLocaleTimeString('de-DE');

  let exported = $state<string | null>(null);
  let exportProblem = $state<string | null>(null);

  async function exportLogs() {
    exportProblem = null;
    try {
      exported = await session.exportLogs();
    } catch (reason) {
      exportProblem = String(reason);
    }
  }
</script>

{#snippet rows(list: Row[])}
  <dl>
    {#each list as row (row.label)}
      <dt>{row.label}</dt>
      <dd class:warn={row.warn}>{row.value}</dd>
    {/each}
  </dl>
{/snippet}

<aside class="debug" aria-label="Debug">
  <section>
    <h2>Verbindung</h2>
    {@render rows([
      { label: 'Status', value: connectionLabel[session.connection] },
      { label: 'Raum', value: session.room ?? '–' },
      { label: 'Ich', value: session.identity ?? '–' },
    ])}
  </section>

  <section>
    <h2>Netz</h2>
    {#if network.length}
      {@render rows(network)}
    {:else}
      <p class="none">Erst im Raum verfügbar.</p>
    {/if}
  </section>

  <section>
    <h2>Mikrofon</h2>
    {@render rows(deviceRows(stats?.capture ?? null, 'Verlorene Samples'))}
  </section>

  <section>
    <h2>Ausgabe</h2>
    {@render rows(deviceRows(stats?.playout ?? null, 'Aussetzer'))}
  </section>

  <section>
    <h2>Engine</h2>
    {@render rows(pipeline)}
  </section>

  <section>
    <h2>Diagnose</h2>
    <button onclick={exportLogs}>Diagnose-Datei speichern</button>
    {#if exported}
      <p class="path" role="status">Gespeichert: {exported}</p>
    {/if}
    {#if exportProblem}
      <p class="warn" role="alert">Speichern fehlgeschlagen: {exportProblem}</p>
    {/if}
    {#if info}
      <p class="none">
        Version {info.version} · Protokoll {info.protocolVersion}
        {#if info.logDir}<br />Logs: <span class="path">{info.logDir}</span>{/if}
      </p>
    {/if}
  </section>

  <section>
    <h2>Verlauf</h2>
    {#if timeline.length === 0}
      <p class="none">Noch keine Einträge.</p>
    {:else}
      <ol class="timeline">
        {#each timeline as entry, index (index)}
          <li class={entry.level}>
            <time>{clock(entry.timeMs)}</time>
            <span>{entry.message}</span>
          </li>
        {/each}
      </ol>
    {/if}
  </section>
</aside>

<style>
  .debug {
    width: 340px;
    overflow-y: auto;
    padding: var(--space-3) var(--space-4);
    background: var(--color-surface);
    border-left: 1px solid var(--color-border);
    font-size: var(--font-size-sm);
    user-select: text;
    scrollbar-color: var(--color-border) transparent;
  }

  section + section {
    margin-top: var(--space-4);
  }

  h2 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-sm);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
  }

  dt {
    color: var(--color-text-muted);
  }

  dd {
    margin: 0;
    text-align: right;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .warn {
    color: var(--color-warning);
  }

  .none {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
  }

  .path {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .timeline {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .timeline li {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2);
    overflow-wrap: anywhere;
  }

  time {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  .timeline .warn span {
    color: var(--color-warning);
  }

  .timeline .error span {
    color: var(--color-danger);
  }
</style>
