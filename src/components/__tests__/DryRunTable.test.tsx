// src/components/__tests__/DryRunTable.test.tsx
import React, { act } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { Step2DryRunTable } from '../Step2DryRunTable';
import { PlannedGame } from '../../types/plan';
import { useIngestionStore } from '../../store/useIngestionStore';

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
    expect(screen.getByText('FF7_1.chd')).toBeDefined();
    const headers = document.querySelectorAll('thead th').length;
    const cells = document.querySelectorAll('tbody tr')[0].querySelectorAll('td').length;
    expect(headers).toBe(cells);
    expect(screen.getByText(/2 Discs/i)).toBeDefined();
    expect(screen.getByText(/95%/i)).toBeDefined();
    expect(screen.getByText(/Review Needed/i)).toBeDefined();
  });

  it('shows a Jev failure note, the release role, and a cue rewrite action', () => {
    const noted = {
      ...mockGames[1],
      status_note: 'Jev classification failed: 401',
      role: 'alternate',
    };
    render(
      <Step2DryRunTable
        games={[noted]}
        onToggleGame={vi.fn()}
        onUpdateTitle={vi.fn()}
        onProceed={vi.fn()}
        onBack={vi.fn()}
      />
    );
    expect(screen.getByText('Jev classification failed: 401')).toBeDefined();
    expect(screen.getByText('alternate')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Accept cue rewrite' })).toBeDefined();
  });

  it('sends the picked release title and region', () => {
    const onApplyRelease = vi.fn();
    render(
      <Step2DryRunTable
        games={[mockGames[1]]}
        onToggleGame={vi.fn()}
        onUpdateTitle={vi.fn()}
        onApplyRelease={onApplyRelease}
        onProceed={vi.fn()}
        onBack={vi.fn()}
      />
    );
    fireEvent.change(screen.getByLabelText('Release title for Unknown Title Hack'), {
      target: { value: 'Real Title' },
    });
    fireEvent.change(screen.getByLabelText('Release region for Unknown Title Hack'), {
      target: { value: 'Europe' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Apply release' }));
    expect(onApplyRelease).toHaveBeenCalledWith('game-2', 'Real Title', 'Europe');
  });

  it('shows the recomputed CHD stem after the title is saved', async () => {
    useIngestionStore.getState().reset();
    useIngestionStore.setState({
      outputDir: 'D:/Out',
      preset: 'esde',
      plan: {
        input_dir: 'D:/In',
        output_dir: 'D:/Out',
        preset: 'esde',
        total_source_bytes: 1,
        estimated_output_bytes: 1,
        games: [
          {
            id: 'g1',
            canonical_title: 'Old Name',
            platform: 'psx',
            region: 'USA',
            is_multidisc: false,
            discs: [
              {
                disc_number: 1,
                source_descriptor: 'old.cue',
                target_chd_path: 'D:/Out/Old Name (USA).chd',
                status: 'pending',
              },
            ],
            target_m3u_path: null,
            confidence: 0.9,
            source: 'fallback',
            enabled: true,
            needs_review: false,
            target_media_paths: ['D:/Out/Old Name (USA).png'],
          },
        ],
      },
    });
    render(<Step2DryRunTable />);
    fireEvent.click(screen.getByTitle('Rename game'));
    fireEvent.change(screen.getByDisplayValue('Old Name'), { target: { value: 'New: Title' } });
    await act(async () => {
      fireEvent.click(screen.getByTitle('Save title'));
    });
    expect(await screen.findByText('D:/Out/esde/New_ Title (USA).chd')).toBeDefined();
    expect(screen.queryByText('D:/Out/Old Name (USA).chd')).toBeNull();
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
