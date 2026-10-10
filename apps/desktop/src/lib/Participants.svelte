<script lang="ts">
  // The others in the room: who speaks, how loud they arrive, and how loud we want them.
  import Meter from './Meter.svelte';
  import type { Session } from './session.svelte';

  let { session }: { session: Session } = $props();
</script>

{#if session.participants.length === 0}
  <p class="empty">Noch niemand sonst da.</p>
{:else}
  <ul class="people">
    {#each session.participants as person (person.identity)}
      {@const level = session.peerLevels[person.identity]}
      {@const localMuted = session.peerMuted[person.identity] ?? false}
      {@const gain = session.peerGainDb[person.identity] ?? 0}
      <li class:speaking={level?.speaking && !localMuted}>
        <div class="who">
          <span class="name">{person.name}</span>
          <span class="state">
            {#if person.muted}
              stumm
            {:else if !person.hasVoice}
              ohne Ton
            {:else if level?.speaking}
              spricht
            {/if}
          </span>
        </div>
        <Meter
          db={level?.levelDb ?? -120}
          active={level?.speaking ?? false}
          label="Pegel {person.name}"
        />
        <div class="mix">
          <input
            type="range"
            min="-30"
            max="12"
            step="1"
            value={gain}
            aria-label="Lautstärke {person.name}"
            oninput={(e) => session.setPeerGain(person.identity, Number(e.currentTarget.value))}
          />
          <span class="db">{gain > 0 ? '+' : ''}{gain} dB</span>
          <button
            class="small"
            aria-pressed={localMuted}
            onclick={() => session.togglePeerMute(person.identity)}
          >
            {localMuted ? 'Stumm für mich' : 'Hörbar'}
          </button>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .empty {
    color: var(--color-text-muted);
  }

  .people {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-3);
  }

  li {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
  }

  li.speaking {
    border-color: var(--color-speaking);
  }

  .who {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
  }

  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state,
  .db {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    white-space: nowrap;
  }

  .mix {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .mix input {
    flex: 1;
    min-width: 0;
    accent-color: var(--color-accent);
  }

  .db {
    width: 4.5em;
    text-align: right;
    font-family: var(--font-mono);
  }

  .small {
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-sm);
  }
</style>
