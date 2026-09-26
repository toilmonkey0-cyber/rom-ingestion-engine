import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, it, expect, beforeEach } from 'vitest';
import { Step3ExecutionProgress } from '../Step3ExecutionProgress';
import { useIngestionStore } from '../../store/useIngestionStore';

describe('Step3ExecutionProgress', () => {
  beforeEach(() => {
    useIngestionStore.getState().reset();
    useIngestionStore.setState({
      isExecuting: true,
      plan: {
        input_dir: 'D:/In',
        output_dir: 'D:/Out',
        preset: 'anbernicstock',
        total_source_bytes: 1000,
        estimated_output_bytes: 500,
        games: [
          {
            id: 'game-1',
            canonical_title: 'Gran Turismo 2',
            platform: 'psx',
            region: 'USA',
            is_multidisc: true,
            discs: [
              { disc_number: 1, source_descriptor: 'GT2_1.cue', target_chd_path: 'GT2_1.chd', status: 'compressing' },
              { disc_number: 2, source_descriptor: 'GT2_2.cue', target_chd_path: 'GT2_2.chd', status: 'pending' },
            ],
            target_m3u_path: 'Gran Turismo 2.m3u',
            confidence: 0.95,
            source: 'redumpcache',
            enabled: true,
            needs_review: false,
          },
        ],
      },
      discProgress: { 'game-1:1': 60, 'game-1:2': 40 },
      activeLogs: ['[10:00:00] Compressing Gran Turismo 2 (Disc 1)...'],
    });
  });

  it('renders overall progress bar, game job card, and terminal logs', () => {
    render(<Step3ExecutionProgress />);

    expect(screen.getByText(/Converting & Verifying Discs/i)).toBeDefined();
    expect(screen.getByText('Gran Turismo 2')).toBeDefined();
    // Per-disc values are averaged per game: (60 + 40) / 2 = 50%
    expect(screen.getAllByText(/50%/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/Compressing Gran Turismo 2/i)).toBeDefined();
    expect(screen.queryByRole('button', { name: /Pause/i })).toBeNull();
  });

  it('renders a failure header (not a green check) when execution errored', () => {
    useIngestionStore.setState({ isExecuting: false, error: 'chdman not found', summary: null });
    render(<Step3ExecutionProgress />);

    expect(screen.getByText(/Execution Failed/i)).toBeDefined();
    expect(screen.queryByText(/Execution Finished/i)).toBeNull();
    // No summary: the View Summary button must not be offered
    expect(screen.queryByRole('button', { name: /View Summary/i })).toBeNull();
    expect(screen.getByRole('button', { name: /Back to Plan/i })).toBeDefined();
  });

  it('offers View Summary only after a summary exists', () => {
    useIngestionStore.setState({ isExecuting: false, error: null, summary: null });
    const { rerender } = render(<Step3ExecutionProgress />);
    expect(screen.queryByRole('button', { name: /View Summary/i })).toBeNull();

    useIngestionStore.setState({
      summary: {
        total_games: 1,
        successful_games: 1,
        failed_games: 0,
        total_discs: 2,
        processed_discs: 2,
        source_files_to_trash: [],
        total_source_bytes: 1000,
        total_output_bytes: 500,
      },
    });
    rerender(<Step3ExecutionProgress />);
    expect(screen.getByRole('button', { name: /View Summary/i })).toBeDefined();
  });
});
