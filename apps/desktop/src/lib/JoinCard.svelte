<script lang="ts">
  // Shown while not in a room: paste the invite, join. The control plane (S6) will replace
  // the pasted text with a login; the engine command behind the button stays the same.
  import type { Session } from './session.svelte';

  let { session }: { session: Session } = $props();
  const connecting = $derived(session.connection === 'connecting');

  function submit(event: SubmitEvent) {
    event.preventDefault();
    void session.join();
  }
</script>

<form class="card" onsubmit={submit}>
  <h1>Beitreten</h1>
  <label for="invite">Einladung</label>
  <textarea
    id="invite"
    rows="4"
    spellcheck="false"
    placeholder="yappa1.…"
    bind:value={session.invite}
    aria-describedby={session.inviteProblem ? 'invite-problem' : undefined}
    aria-invalid={session.inviteProblem ? 'true' : undefined}></textarea>
  {#if session.inviteProblem}
    <p id="invite-problem" class="problem" role="alert">{session.inviteProblem}</p>
  {/if}
  <button type="submit" class="primary" disabled={connecting || session.invite.trim() === ''}>
    {connecting ? 'Verbinde …' : 'Beitreten'}
  </button>
  <p class="hint">
    Die Einladung bekommst du von der Person, die den Server betreibt. Sie wird auf diesem Rechner
    gemerkt.
  </p>
</form>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 460px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  textarea {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
    resize: vertical;
    user-select: text;
    word-break: break-all;
  }

  textarea[aria-invalid='true'] {
    border-color: var(--color-danger);
  }

  .primary {
    align-self: flex-start;
    margin-top: var(--space-2);
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-accent-text);
    font-weight: 600;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .problem {
    margin: 0;
    color: var(--color-danger);
  }

  .hint {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
