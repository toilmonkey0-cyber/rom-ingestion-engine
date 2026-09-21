import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, beforeEach } from 'vitest';
import { Step1Config } from '../Step1Config';
import { useIngestionStore } from '../../store/useIngestionStore';

describe('Step1Config', () => {
  beforeEach(() => {
    useIngestionStore.getState().reset();
  });

  it('renders presets with Anbernic Stock OS, OnionOS, and folder inputs', () => {
    render(<Step1Config />);

    expect(screen.getAllByText('Anbernic Stock OS').length).toBeGreaterThan(0);
    expect(screen.getAllByText('OnionOS / GarlicOS').length).toBeGreaterThan(0);
    expect(screen.getByText('ES-DE (EmulationStation)')).toBeDefined();
    expect(screen.getAllByText('Batocera / Knulli').length).toBeGreaterThan(0);
    expect(screen.getByPlaceholderText(/e\.g\. D:\/Emulation\/Dumps/i)).toBeDefined();
  });

  it('updates input directory in store when typing', () => {
    render(<Step1Config />);

    const input = screen.getByPlaceholderText(/e\.g\. D:\/Emulation\/Dumps/i);
    fireEvent.change(input, { target: { value: 'C:/Roms/PSX_Dumps' } });

    expect(useIngestionStore.getState().inputDir).toBe('C:/Roms/PSX_Dumps');
  });

  it('switches preset when clicking a preset card', () => {
    render(<Step1Config />);

    // The preset name also appears in the migration panel's select — click
    // the card instance specifically.
    const onionCard = screen
      .getAllByText('OnionOS / GarlicOS')
      .find((el) => el.closest('[role="button"]')) as HTMLElement;
    fireEvent.click(onionCard);

    expect(useIngestionStore.getState().preset).toBe('onionos');
  });

  it('renders chdman readiness banner in Step 1', async () => {
    render(<Step1Config />);

    // Should display chdman banner in missing state by default
    expect(await screen.findByText(/chdman Required/i)).toBeDefined();
    expect(screen.getByText(/Install chdman \(1-Click\)/i)).toBeDefined();
  });
});
