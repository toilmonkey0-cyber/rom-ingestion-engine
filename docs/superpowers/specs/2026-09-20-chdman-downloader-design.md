# Bundled chdman 1-Click Auto-Downloader & Sidecar Manager - Architecture & Design Specification

**Date:** 2026-09-20  
**Status:** Approved  
**Target Platform:** Desktop (Tauri v2: Rust + React 19 / TypeScript / Tailwind CSS)

---

## 1. Overview & Problem Statement

The Intelligent ROM Library Ingestion Engine relies on MAME's `chdman` tool to compress uncompressed disc dumps (`.bin/.cue`, `.iso`, `.gdi`) into lossless `.chd` files. Requiring users to manually locate, download, extract, and add `chdman.exe` to their system environment variables creates unnecessary friction.

This specification defines a **1-Click Auto-Downloader and Tool Manager**:
1. Automatically detects `chdman` on system `PATH`, in user custom paths, or in `<AppLocalData>/bin/`.
2. Provides a zero-configuration "1-Click Install" button directly inside the Step 1 configuration view.
3. Streams platform-specific binaries for Windows (x86_64), macOS (Apple Silicon ARM64 & Intel x86_64), and Linux (x86_64).
4. Verifies cryptographic SHA-256 integrity before promoting the binary.
5. Sets executable bits (`0o755`) on Unix systems and tests binary execution before marking readiness.
6. Allows power users to manually browse and select their own existing binary.

---

## 2. Platform Manifest & Binary Sourcing

### 2.1 Manifest Structure
In Rust (`src-tauri/src/chdman/downloader.rs`), a platform manifest maps OS and CPU architecture to release assets:

```rust
pub struct PlatformManifest {
    pub os: &'static str,
    pub arch: &'static str,
    pub download_url: &'static str,
    pub expected_sha256: &'static str,
    pub archive_format: ArchiveFormat, // Zip | TarGz | RawBinary
    pub binary_name: &'static str,     // "chdman.exe" on Windows, "chdman" on Unix
}
```

### 2.2 Storage Directory
Installed binaries are placed in standard, user-writable application local data directories:
- **Windows:** `%LOCALAPPDATA%\rom-ingestion-engine\bin\chdman.exe`
- **macOS:** `~/Library/Application Support/rom-ingestion-engine/bin/chdman`
- **Linux:** `~/.local/share/rom-ingestion-engine/bin/chdman`

---

## 3. Execution Pipeline & Security

```
[ Frontend: Click "Install chdman" ]
                │
                ▼
[ Tauri Command: download_chdman ]
                │
                ├─► 1. Fetch Content-Length, stream chunks (64 KB)
                ├─► 2. Write to `<AppLocalData>/bin/chdman_download.tmp`
                ├─► 3. Hash with `sha2::Sha256` in real-time
                ├─► 4. Emit `DownloadProgressEvent { downloaded, total, pct }`
                │
                ▼
[ Checksum Verification ]
    ├── Match? ────► Extract / Move to `<AppLocalData>/bin/chdman[.exe]`
    │                Set Unix permissions (0o755)
    │                Run `chdman --version` to test launch
    │                Return `ChdmanStatus::Ready`
    │
    └── Mismatch? ─► Purge `.tmp` file
                     Return `ChecksumMismatch` error
```

### 3.1 Non-Destructive Invariants
- Temporary files use a `.tmp` suffix during download.
- If download fails, is cancelled, or the SHA-256 hash does not match, the temporary file is immediately deleted.
- Existing custom binaries configured by the user are never overwritten.

---

## 4. Tauri IPC Interface & Data Models

### 4.1 Data Structures
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChdmanStatus {
    pub ready: bool,
    pub source: ChdmanSource, // SystemPath | ManagedDirectory | CustomPath | Missing
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChdmanSource {
    SystemPath,
    ManagedDirectory,
    CustomPath,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
}
```

### 4.2 Commands
1. `check_chdman_status() -> Result<ChdmanStatus, String>`: Resolves current status across search priority.
2. `download_chdman() -> Result<ChdmanStatus, String>`: Initiates download, streams progress events, verifies checksum, installs, and returns status.
3. `set_custom_chdman_path(path: String) -> Result<ChdmanStatus, String>`: Validates user-selected binary and persists path.

---

## 5. UI Integration (Step 1)

### 5.1 Banner States
1. **Ready State (Green):**
   - Displays green checkmark, active path, and detected version.
   - Includes a secondary "Change..." button to browse for a different binary.
2. **Missing State (Amber):**
   - Displays alert banner: *"chdman is required for disc compression."*
   - Two buttons: `[ Install chdman (1-Click) ]` and `[ Browse Local File... ]`.
3. **Downloading State (Active Progress):**
   - Replaces buttons with an active progress bar: percentage, downloaded MB / total MB.
4. **Error State (Red):**
   - Displays error description (e.g. network timeout or checksum failure) with a `[ Retry ]` button.

---

## 6. Verification & Testing

### 6.1 Unit & Integration Tests (Rust)
- Platform manifest resolution for current OS/Arch.
- SHA-256 checksum streaming calculation.
- Mock HTTP server download test verifying progress event emission, extraction, and checksum validation.
- Checksum mismatch error handling and cleanup test.
- Version extraction test (`chdman --version` output parsing).

### 6.2 Frontend Tests (Vitest)
- Render banner in Missing state.
- Trigger download and verify progress bar rendering.
- Render banner in Ready state.
- Manual file selection and path update.
