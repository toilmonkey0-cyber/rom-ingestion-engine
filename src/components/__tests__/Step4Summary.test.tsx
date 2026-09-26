import React, { act } from 'react';
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

  it('renders a finished banner, output written, and stats', () => {
    render(<Step4Summary />);

    expect(screen.getByText('Ingestion finished')).toBeDefined();
    expect(screen.queryByText(/100% Verified/i)).toBeNull();
    expect(screen.queryByText(/bit-perfect/i)).toBeNull();
    expect(screen.getAllByText('2.33 GB').length).toBeGreaterThan(0);
    expect(screen.getByText('5')).toBeDefined();
    expect(screen.getByText('8')).toBeDefined();
    expect(screen.queryByText(/Disk space freed/i)).toBeNull();
  });

  it('hides the success banner and locks trash when a game failed', () => {
    useIngestionStore.setState({
      summary: {
        total_games: 5,
        successful_games: 4,
        failed_games: 1,
        total_discs: 8,
        processed_discs: 4,
        source_files_to_trash: ['FF7_1.cue', 'FF7_1.bin'],
        total_source_bytes: 5000000000,
        total_output_bytes: 2500000000,
      },
    });

    render(<Step4Summary />);

    expect(screen.getByText('Ingestion finished with failures')).toBeDefined();
    expect(screen.queryByText('Ingestion finished')).toBeNull();
    expect(screen.queryByText(/100% Verified/i)).toBeNull();
    expect(screen.queryByText(/bit-perfect/i)).toBeNull();
    expect(screen.getByRole('button', { name: /Move Source Dumps to Trash/i })).toBeDisabled();
  });

  it('lists failed and partial multi-disc sets and shows freed bytes only after recycle', () => {
    useIngestionStore.setState({
      summary: {
        total_games: 2,
        successful_games: 0,
        failed_games: 2,
        total_discs: 4,
        processed_discs: 1,
        source_files_to_trash: [],
        total_source_bytes: 5000000000,
        total_output_bytes: 100,
        failed_game_ids: ['alpha-set'],
        partial_game_ids: ['alpha-set'],
      },
      recycledBytes: null,
    });
    const { rerender } = render(<Step4Summary />);
    expect(screen.getByText('Failed games')).toBeDefined();
    expect(screen.getByText('Partial multi-disc sets')).toBeDefined();
    expect(screen.getAllByText('alpha-set')).toHaveLength(2);
    expect(screen.queryByText(/Disk space freed/i)).toBeNull();
    expect(screen.queryByText(/100% Verified/i)).toBeNull();
    expect(screen.queryByText(/bit-perfect/i)).toBeNull();

    act(() => {
      useIngestionStore.setState({ trashedCount: 3, recycledBytes: 4096 });
    });
    rerender(<Step4Summary />);
    expect(screen.getByText('Disk space freed')).toBeDefined();
    expect(screen.getByText('4.00 KB')).toBeDefined();
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
