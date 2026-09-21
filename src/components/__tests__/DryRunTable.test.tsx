// src/components/__tests__/DryRunTable.test.tsx
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { Step2DryRunTable } from '../Step2DryRunTable';
import { PlannedGame } from '../../types/plan';

const mockGames: PlannedGame[] = [
  {
    id: 'game-1',
    canonical_title: 'Final Fantasy VII',
    platform: 'psx',
    region: 'USA',
    is_multidisc: true,
    discs: [
      { disc_number: 1, source_descriptor: 'FF7_1.cue', target_chd_path: 'FF7_1.chd', status: 'pending' },
      { disc_number: 2, source_descriptor: 'FF7_2.cue', target_chd_path: 'FF7_2.chd', status: 'pending' },
    ],
    target_m3u_path: 'Final Fantasy VII.m3u',
    confidence: 0.95,
    source: 'jevai',
    enabled: true,
    needs_review: false,
  },
  {
    id: 'game-2',
    canonical_title: 'Unknown Title Hack',
    platform: 'psx',
    region: 'UNKNOWN',
    is_multidisc: false,
    discs: [
      { disc_number: 1, source_descriptor: 'hack.cue', target_chd_path: 'hack.chd', status: 'pending' },
    ],
    target_m3u_path: null,
    confidence: 0.65,
    source: 'jevai',
    enabled: true,
    needs_review: true,
  },
];

describe('Step2DryRunTable', () => {
  it('renders game title, badges, and disc count', () => {
    render(
      <Step2DryRunTable
        games={mockGames}
        onToggleGame={vi.fn()}
        onUpdateTitle={vi.fn()}
        onProceed={vi.fn()}
        onBack={vi.fn()}
      />
    );
    expect(screen.getByText('Final Fantasy VII')).toBeDefined();
    expect(screen.getByText(/2 Discs/i)).toBeDefined();
    expect(screen.getByText(/95%/i)).toBeDefined();
    expect(screen.getByText(/Review Needed/i)).toBeDefined();
  });

  it('triggers onToggleGame when checkbox is clicked', () => {
    const handleToggle = vi.fn();
    render(
      <Step2DryRunTable
        games={mockGames}
        onToggleGame={handleToggle}
        onUpdateTitle={vi.fn()}
        onProceed={vi.fn()}
        onBack={vi.fn()}
      />
    );

    const checkbox = screen.getByLabelText('Toggle Final Fantasy VII');
    fireEvent.click(checkbox);
    expect(handleToggle).toHaveBeenCalledWith('game-1');
  });

  it('triggers onProceed when Start Ingestion button is clicked', () => {
    const handleProceed = vi.fn();
    render(
      <Step2DryRunTable
        games={mockGames}
        onToggleGame={vi.fn()}
        onUpdateTitle={vi.fn()}
        onProceed={handleProceed}
        onBack={vi.fn()}
      />
    );

    const proceedBtn = screen.getByRole('button', { name: /Start Ingestion/i });
    fireEvent.click(proceedBtn);
    expect(handleProceed).toHaveBeenCalled();
  });

  it('allows inline editing of canonical title', () => {
    const handleUpdateTitle = vi.fn();
    render(
      <Step2DryRunTable
        games={mockGames}
        onToggleGame={vi.fn()}
        onUpdateTitle={handleUpdateTitle}
        onProceed={vi.fn()}
        onBack={vi.fn()}
      />
    );

    const editBtn = screen.getAllByTitle('Rename game')[0];
    fireEvent.click(editBtn);

    const input = screen.getByDisplayValue('Final Fantasy VII');
    fireEvent.change(input, { target: { value: 'Final Fantasy VII (International)' } });

    const saveBtn = screen.getByTitle('Save title');
    fireEvent.click(saveBtn);

    expect(handleUpdateTitle).toHaveBeenCalledWith('game-1', 'Final Fantasy VII (International)');
  });
});
