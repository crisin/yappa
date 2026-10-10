<script lang="ts">
  // A level bar from -60 dBFS to full scale. `active` colours it (speaking, sending).
  import { meterFill } from './session.svelte';

  let { db, active = true, label }: { db: number; active?: boolean; label: string } = $props();
  const shown = $derived(Math.max(-60, Math.round(db)));
</script>

<div
  class="meter"
  class:active
  role="meter"
  aria-label={label}
  aria-valuemin={-60}
  aria-valuemax={0}
  aria-valuenow={shown}
>
  <div class="fill" style:transform="scaleX({meterFill(db)})"></div>
</div>

<style>
  .meter {
    height: 6px;
    border-radius: var(--radius-sm);
    background: var(--color-surface-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--color-text-muted);
    transform-origin: left;
    transition: transform 80ms linear;
  }

  .active .fill {
    background: var(--color-speaking);
  }
</style>
