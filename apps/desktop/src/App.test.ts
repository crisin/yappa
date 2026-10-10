import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App.svelte';
import { encodeInvite } from './lib/mockEngine';

const invite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

beforeEach(() => localStorage.clear());

async function join() {
  await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: invite } });
  await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
  await screen.findByRole('heading', { level: 1, name: 'gang' });
}

describe('App', () => {
  it('starts with the join card, the logo and the device choice', async () => {
    const { container } = render(App);
    expect(container.querySelector('header img')).toHaveAttribute('src');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Beitreten');
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent('Nicht verbunden');
    expect(await screen.findByRole('option', { name: 'Webcam' })).toBeInTheDocument();
    expect(screen.getByRole('meter', { name: 'Mikrofonpegel' })).toBeInTheDocument();
  });

  it('explains a pasted text that is not an invite', async () => {
    render(App);
    await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: 'hallo' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('keine yAPPA-Einladung');
    expect(screen.getByRole('status')).toHaveTextContent('Nicht verbunden');
  });

  it('joins with an invite and shows who is there', async () => {
    render(App);
    await join();
    expect(screen.getByRole('status')).toHaveTextContent('Verbunden');
    const people = screen.getAllByRole('listitem');
    expect(people).toHaveLength(2);
    expect(within(people[0]).getByText('Anna')).toBeInTheDocument();
    expect(within(people[1]).getByText('stumm')).toBeInTheDocument();
    expect(screen.getByRole('slider', { name: 'Lautstärke Anna' })).toHaveValue('0');
  });

  it('leaves the room and is back at the join card with the invite remembered', async () => {
    render(App);
    await join();
    await fireEvent.click(screen.getByRole('button', { name: 'Verlassen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Beitreten' })).toBeInTheDocument();
    expect(screen.getByLabelText('Einladung')).toHaveValue(invite);
    expect(screen.queryByRole('button', { name: 'Verlassen' })).not.toBeInTheDocument();
  });

  it('toggles mute and tells the user the microphone is off', async () => {
    render(App);
    await fireEvent.click(await screen.findByRole('button', { name: 'Mikro an' }));
    expect(screen.getByRole('button', { name: 'Mikro aus' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('mutes one person for me only', async () => {
    render(App);
    await join();
    const anna = screen.getAllByRole('listitem')[0];
    await fireEvent.click(within(anna).getByRole('button', { name: 'Hörbar' }));
    expect(within(anna).getByRole('button', { name: 'Stumm für mich' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('shows the debug panel on demand, with the timeline', async () => {
    render(App);
    expect(screen.queryByRole('complementary', { name: 'Debug' })).not.toBeInTheDocument();
    await join();
    await fireEvent.click(screen.getByRole('button', { name: 'Debug' }));
    const panel = screen.getByRole('complementary', { name: 'Debug' });
    expect(within(panel).getByText('verbunden')).toBeInTheDocument();
    expect(within(panel).getByText('gang')).toBeInTheDocument();
    expect(within(panel).getByText(/joining wss:\/\/voice\.example/)).toBeInTheDocument();
    await fireEvent.click(within(panel).getByRole('button', { name: 'Diagnose-Datei speichern' }));
    expect(await within(panel).findByText(/Gespeichert:/)).toBeInTheDocument();
  });
});
