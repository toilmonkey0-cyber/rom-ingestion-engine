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
      gameProgress: { 'game-1': 50 },
      activeLogs: ['[10:00:00] Compressing Gran Turismo 2 (Disc 1)...'],
    });
  });

  it('renders overall progress bar, game job card, and terminal logs', () => {
    render(<Step3ExecutionProgress />);

    expect(screen.getByText(/Converting & Verifying Discs/i)).toBeDefined();
    expect(screen.getByText('Gran Turismo 2')).toBeDefined();
    expect(screen.getAllByText(/50%/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/Compressing Gran Turismo 2/i)).toBeDefined();
  });
});
