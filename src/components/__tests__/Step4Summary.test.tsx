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

    expect(screen.getByText(/Verified & Validated/i)).toBeDefined();
    expect(screen.getByText('50%')).toBeDefined(); // 5GB -> 2.5GB = 50%
    expect(screen.getByText('5')).toBeDefined(); // 5 games
    expect(screen.getByText('8')).toBeDefined(); // 8 discs
  });

  it('renders partial-failure hero when some games failed', () => {
    useIngestionStore.setState({
      summary: {
        total_games: 5,
        successful_games: 3,
        failed_games: 2,
        total_discs: 8,
        processed_discs: 5,
        source_files_to_trash: ['FF7_1.cue', 'FF7_1.bin'],
        total_source_bytes: 5000000000,
        total_output_bytes: 2500000000,
      },
    });
    render(<Step4Summary />);

    expect(screen.getByText(/Completed with Failures/i)).toBeDefined();
    expect(screen.getByText(/3 Succeeded, 2 Failed/i)).toBeDefined();
    expect(screen.getByText('2')).toBeDefined(); // failed-games tile
    expect(screen.queryByText(/Ingestion Completed Successfully/i)).toBeNull();
  });

  it('renders total-failure hero when no games succeeded', () => {
    useIngestionStore.setState({
      summary: {
        total_games: 5,
        successful_games: 0,
        failed_games: 5,
        total_discs: 8,
        processed_discs: 0,
        source_files_to_trash: [],
        total_source_bytes: 5000000000,
        total_output_bytes: 0,
      },
    });
    render(<Step4Summary />);

    expect(screen.getByText(/Ingestion Failed/i)).toBeDefined();
    expect(screen.getByText(/No Games Ingested/i)).toBeDefined();
    // No files to trash: cleanup button disabled
    expect(
      screen.getByRole('button', { name: /Move Source Dumps to Trash/i }).hasAttribute('disabled')
    ).toBe(true);
  });

  it('shows trash errors inside the modal instead of behind the overlay', async () => {
    const trashSpy = vi.fn().mockRejectedValue(new Error("Refusing to trash 'x.exe': not a disc-image file"));
    useIngestionStore.setState({ trashSourceFiles: trashSpy });

    render(<Step4Summary />);

    fireEvent.click(screen.getByRole('button', { name: /Move Source Dumps to Trash/i }));

    const confirmBtn = screen.getByRole('button', { name: /Confirm Move to Trash/i });
    await React.act(async () => {
      fireEvent.click(confirmBtn);
    });

    expect(screen.getByText(/Nothing was moved to the trash/i)).toBeDefined();
    expect(screen.getByText(/not a disc-image file/i)).toBeDefined();
    // Modal stays open so the error is visible
    expect(screen.getByText(/Move Source Dumps to Recycle Bin\?/i)).toBeDefined();
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
