import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { ChdmanStatusBanner } from '../ChdmanStatusBanner';
import { ChdmanStatus } from '../../types/plan';

describe('ChdmanStatusBanner', () => {
  it('renders missing state with 1-click install and browse buttons', () => {
    const status: ChdmanStatus = { ready: false, source: 'missing', path: null, version: null };
    const onInstall = vi.fn();
    const onBrowse = vi.fn();
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={false}
        downloadProgress={0}
        error={null}
        onInstall={onInstall}
        onBrowse={onBrowse}
      />
    );
    expect(screen.getByText(/chdman Required/i)).toBeDefined();
    expect(screen.getByText(/Install chdman \(1-Click\)/i)).toBeDefined();
    expect(screen.getByText(/Browse Local File/i)).toBeDefined();

    fireEvent.click(screen.getByText(/Install chdman \(1-Click\)/i));
    expect(onInstall).toHaveBeenCalled();
  });

  it('renders ready state with green badge and path', () => {
    const status: ChdmanStatus = {
      ready: true,
      source: 'managed_directory',
      path: 'C:/Tools/chdman.exe',
      version: '0.268',
    };
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={false}
        downloadProgress={0}
        error={null}
        onInstall={vi.fn()}
        onBrowse={vi.fn()}
      />
    );
    expect(screen.getByText(/Compression Engine Ready/i)).toBeDefined();
    expect(screen.getByText(/C:\/Tools\/chdman.exe/i)).toBeDefined();
  });

  it('renders downloading state with progress bar', () => {
    const status: ChdmanStatus = { ready: false, source: 'missing', path: null, version: null };
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={true}
        downloadProgress={64.5}
        error={null}
        onInstall={vi.fn()}
        onBrowse={vi.fn()}
      />
    );
    expect(screen.getByText(/Downloading & Verifying chdman/i)).toBeDefined();
    expect(screen.getByText(/65%/i)).toBeDefined();
  });

  it('renders error state with retry button and error message', () => {
    const status: ChdmanStatus = { ready: false, source: 'missing', path: null, version: null };
    const onRetry = vi.fn();
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={false}
        downloadProgress={0}
        error="Failed to verify SHA-256 checksum"
        onInstall={vi.fn()}
        onBrowse={vi.fn()}
        onRetry={onRetry}
      />
    );
    expect(screen.getByText(/Failed to verify SHA-256 checksum/i)).toBeDefined();
    const retryBtn = screen.getByRole('button', { name: /retry/i });
    expect(retryBtn).toBeDefined();
    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalled();
  });

  it('calls onBrowse or onChangePath when Change button clicked in ready state', () => {
    const status: ChdmanStatus = {
      ready: true,
      source: 'system_path',
      path: '/usr/bin/chdman',
      version: '0.260',
    };
    const onBrowse = vi.fn();
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={false}
        downloadProgress={0}
        error={null}
        onInstall={vi.fn()}
        onBrowse={onBrowse}
      />
    );
    const changeBtn = screen.getByRole('button', { name: /change/i });
    expect(changeBtn).toBeDefined();
    fireEvent.click(changeBtn);
    expect(onBrowse).toHaveBeenCalled();
  });
});
