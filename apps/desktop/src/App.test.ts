import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import App from './App.svelte';

describe('App', () => {
  it('lists the channels and opens the first one', () => {
    render(App);
    expect(screen.getByRole('navigation', { name: 'Channels' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Lobby');
  });

  it('switches channel on click', async () => {
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Zocken/ }));
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Zocken');
  });

  it('has exactly one active transmit mode', async () => {
    render(App);
    await fireEvent.click(await screen.findByRole('button', { name: 'Push-to-Talk' }));
    const pressed = screen
      .getAllByRole('button')
      .filter((b) => b.getAttribute('aria-pressed') === 'true');
    expect(pressed.map((b) => b.textContent?.trim())).toEqual(['Push-to-Talk']);
  });

  it('toggles mute', async () => {
    render(App);
    await fireEvent.click(await screen.findByRole('button', { name: 'Mikro an' }));
    expect(screen.getByRole('button', { name: 'Mikro aus' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});
