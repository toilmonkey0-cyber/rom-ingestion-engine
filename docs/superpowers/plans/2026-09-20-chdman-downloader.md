# Bundled chdman 1-Click Auto-Downloader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a 1-click auto-downloader, SHA-256 checksum verifier, and sidecar manager for MAME's `chdman` tool with an interactive readiness banner in the desktop UI.

**Architecture:** A Rust module (`src-tauri/src/chdman/downloader.rs`) resolves platform download manifests, streams the archive/binary into `<AppLocalData>/bin/`, validates cryptographic SHA-256 checksums, sets executable permissions, and checks version outputs. Tauri IPC commands (`check_chdman_status`, `download_chdman`, `set_custom_chdman_path`) connect to a React readiness banner in Step 1 with live download progress bars and custom file browsing.

**Tech Stack:** Rust (reqwest, sha2, zip, flate2, tar), Tauri v2, React 19, TypeScript, Tailwind CSS, Lucide React, Zustand.

## Global Constraints
- Supported target platforms: Windows (x86_64), macOS (Apple Silicon ARM64 & Intel x86_64), Linux (x86_64).
- Safe temporary downloads: write to `.tmp` file and delete immediately on failure or checksum mismatch.
- SHA-256 cryptographic verification is mandatory before binary installation.
- Unix executable permissions (`0o755`) set on macOS and Linux.
- Step 1 UI must feature an interactive status banner: Ready (green), Missing (amber with 1-Click Download and Browse buttons), Downloading (progress bar), Error (red with retry).

---

### Task 1: Platform Manifest, SHA-256 Hasher & Storage Path Resolver

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `sha2`, `zip`, `flate2`, `tar`)
- Create: `src-tauri/src/chdman/downloader.rs`
- Modify: `src-tauri/src/chdman/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/downloader_test.rs`

**Interfaces:**
- Produces:
  - `PlatformManifest`, `ArchiveFormat`
  - `get_platform_manifest(os: &str, arch: &str) -> Option<PlatformManifest>`
  - `get_managed_tools_dir() -> Result<PathBuf, std::io::Error>`
  - `compute_file_sha256<P: AsRef<Path>>(path: P) -> Result<String, std::io::Error>`
  - `extract_archive<P: AsRef<Path>>(archive_path: P, format: ArchiveFormat, dest_dir: P, binary_name: &str) -> Result<PathBuf, DownloadError>`

- [ ] **Step 1: Write failing test for manifest resolution, hash calculation, and archive extraction**

```rust
// src-tauri/tests/downloader_test.rs
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use rom_ingest_core::chdman::downloader::*;

#[test]
fn test_platform_manifest_resolution() {
    let win_manifest = get_platform_manifest("windows", "x86_64");
    assert!(win_manifest.is_some());
    let m = win_manifest.unwrap();
    assert_eq!(m.binary_name, "chdman.exe");
    assert!(!m.download_url.is_empty());
    assert_eq!(m.expected_sha256.len(), 64);

    let linux_manifest = get_platform_manifest("linux", "x86_64");
    assert!(linux_manifest.is_some());
    assert_eq!(linux_manifest.unwrap().binary_name, "chdman");
}

#[test]
fn test_compute_sha256() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.bin");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(b"test data for sha256 calculation").unwrap();

    let hash = compute_file_sha256(&file_path).unwrap();
    assert_eq!(hash.len(), 64);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test downloader_test`
Expected: FAIL

- [ ] **Step 3: Implement `downloader.rs` manifest, hashing, and path resolver**

Implement manifest lookup table with SHA-256 verification and cross-platform app data path resolution.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test downloader_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat(chdman): implement platform manifest and sha256 verifier"
```

---

### Task 2: Streaming Download, Checksum Verification & Tauri IPC Commands

**Files:**
- Modify: `src-tauri/src/chdman/downloader.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/downloader_test.rs`

**Interfaces:**
- Produces:
  - `ChdmanStatus`, `ChdmanSource`, `DownloadProgressEvent`
  - `detect_chdman(custom_path: Option<&str>) -> ChdmanStatus`
  - Tauri Commands in `commands.rs`:
    - `check_chdman_status(custom_path: Option<String>) -> Result<ChdmanStatus, String>`
    - `download_chdman(app_handle: AppHandle) -> Result<ChdmanStatus, String>`
    - `set_custom_chdman_path(path: String) -> Result<ChdmanStatus, String>`

- [ ] **Step 1: Write failing test for download streaming and checksum mismatch rejection**

```rust
// In src-tauri/tests/downloader_test.rs
#[tokio::test]
async fn test_checksum_mismatch_purges_temporary_file() {
    let dir = tempdir().unwrap();
    let dest_dir = dir.path().to_path_buf();
    let res = verify_and_install_download(
        &dest_dir.join("corrupt.tmp"),
        "expected_real_sha256_00000000000000000000000000000000000000000000000",
        ArchiveFormat::RawBinary,
        &dest_dir,
        "chdman.exe"
    );
    assert!(res.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test downloader_test`
Expected: FAIL

- [ ] **Step 3: Implement streaming downloader and Tauri IPC commands**

Implement `download_and_install_chdman` with streaming chunk hashing, progress emission, and IPC commands in `commands.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test downloader_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat(chdman): implement streaming download, checksum verification, and tauri commands"
```

---

### Task 3: Frontend UI Readiness Banner, Zustand Store & Tests

**Files:**
- Create: `src/components/ChdmanStatusBanner.tsx`
- Modify: `src/types/plan.ts`
- Modify: `src/store/useIngestionStore.ts`
- Modify: `src/components/Step1Config.tsx`
- Test: `src/components/__tests__/ChdmanStatusBanner.test.tsx`

**Interfaces:**
- Produces:
  - `ChdmanStatus`, `ChdmanSource`, `DownloadProgressEvent` in `types/plan.ts`
  - Store actions: `checkChdmanStatus`, `downloadChdman`, `setCustomChdmanPath`
  - `ChdmanStatusBanner` component rendering Ready, Missing, Downloading, and Error states.

- [ ] **Step 1: Write failing test for `ChdmanStatusBanner` component**

```tsx
// src/components/__tests__/ChdmanStatusBanner.test.tsx
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { ChdmanStatusBanner } from '../ChdmanStatusBanner';
import { ChdmanStatus } from '../../types/plan';

describe('ChdmanStatusBanner', () => {
  it('renders missing state with 1-click install and browse buttons', () => {
    const status: ChdmanStatus = { ready: false, source: 'missing', path: null, version: null };
    render(
      <ChdmanStatusBanner
        status={status}
        isDownloading={false}
        downloadProgress={0}
        onInstall={vi.fn()}
        onBrowse={vi.fn()}
      />
    );
    expect(screen.getByText(/chdman Required/i)).toBeDefined();
    expect(screen.getByText(/Install chdman \(1-Click\)/i)).toBeDefined();
    expect(screen.getByText(/Browse Local File/i)).toBeDefined();
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
        onInstall={vi.fn()}
        onBrowse={vi.fn()}
      />
    );
    expect(screen.getByText(/Compression Engine Ready/i)).toBeDefined();
    expect(screen.getByText(/C:\/Tools\/chdman.exe/i)).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL

- [ ] **Step 3: Implement `ChdmanStatusBanner` and integrate into Step 1**

Implement component with Tailwind dark aesthetic, integrate into `Step1Config.tsx`, and hook store actions to Tauri IPC.

- [ ] **Step 4: Run frontend tests to verify they pass**

Run: `npm test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(ui): implement chdman status readiness banner and download progress"
```

---

### Task 4: End-to-End Verification & Health Check

**Files:**
- Modify: `src-tauri/tests/e2e_pipeline_test.rs`
- Run full test suites (`cargo test` & `npm test`)
- Run production build (`npm run build`)

- [ ] **Step 1: Run full test suite and verify 0 errors**

Run: `cargo test && npm test && npm run build`
Expected: PASS with 0 errors

- [ ] **Step 2: Commit**

```bash
git add .
git commit -m "feat(pipeline): complete chdman downloader integration and verification"
```
