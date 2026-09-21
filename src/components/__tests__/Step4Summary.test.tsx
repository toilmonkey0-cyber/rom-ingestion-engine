import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { Step4Summary } from '../Step4Summary';
import { useIngestionStore } from '../../store/useIngestionStore';

describe('Step4Summary', () => {
  beforeEach(() => {
    useIngestionStore.getState().reset();
    useIngestionStore.setState({
      summary: {
        total_games: 5,
        successful_games: 5,
        failed_games: 0,
        total_discs: 8,
        processed_discs: 8,
        source_files_to_trash: ['FF7_1.cue', 'FF7_1.bin', 'FF7_2.cue', 'FF7_2.bin'],
        total_source_bytes: 5000000000,
        total_output_bytes: 2500000000,
      },
    });
  });

  it('renders verified banner, space saved percentage, and stats', () => {
    render(<Step4Summary />);

    expect(screen.getByText(/100% Verified & Validated/i)).toBeDefined();
    expect(screen.getByText('50%')).toBeDefined(); // 5GB -> 2.5GB = 50%
    expect(screen.getByText('5')).toBeDefined(); // 5 games
    expect(screen.getByText('8')).toBeDefined(); // 8 discs
  });

  it('opens confirmation modal and handles trash action', async () => {
    const trashSpy = vi.fn().mockResolvedValue(4);
    useIngestionStore.setState({ trashSourceFiles: trashSpy });

    render(<Step4Summary />);

    const openModalBtn = screen.getByRole('button', { name: /Move Source Dumps to Trash/i });
    fireEvent.click(openModalBtn);

    expect(screen.getByText(/Move Source Dumps to Recycle Bin\?/i)).toBeDefined();
    expect(screen.getByText(/These files are NOT permanently deleted/i)).toBeDefined();

    const confirmBtn = screen.getByRole('button', { name: /Confirm Move to Trash/i });
    await React.act(async () => {
      fireEvent.click(confirmBtn);
    });

    expect(trashSpy).toHaveBeenCalled();
  });
});
