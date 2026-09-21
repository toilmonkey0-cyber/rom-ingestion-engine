# Intelligent ROM Library Ingestion Engine - Architecture & Design Specification

**Date:** 2026-09-20  
**Status:** Approved  
**Target Platform:** Desktop (Tauri v2: Rust + React / TypeScript / Tailwind CSS)

---

## 1. Executive Summary & Problem Statement

Retro gaming enthusiasts and handheld owners (Miyoo Mini, Anbernic, Steam Deck, Batocera consoles) frequently face messy, unorganized ROM dumps consisting of multi-track `.bin` files, missing or malformed `.cue` sheets, and unmanaged multi-disc releases. Loading these raw files into modern frontends (ES-DE, OnionOS, Anbernic Stock OS, Batocera) results in duplicated entries, missing scrapers, and wasted SD card space.

The **Intelligent ROM Library Ingestion Engine** is a cross-platform desktop application that automates this workflow:
1. Discovers and pairs disc-based game dumps (`.bin/.cue`, `.iso`, `.gdi`).
2. Classifies messy filenames into canonical metadata using a two-tier engine: an embedded fast Redump hash database backed by the **TypeSafe Jev System One API** (`jev-latest`).
3. Compresses uncompressed disc dumps into lossless `.chd` archives via an integrated `chdman` worker queue with real-time progress streaming.
4. Organizes multi-disc sets into clean, frontend-specific directory hierarchies with auto-generated `.m3u` playlists and hidden disc subfolders.
5. Provides an immutable, interactive **Dry-Run Review** before modifying disk storage, guaranteeing non-destructive processing and data safety.

---

## 2. Tech Stack & Architecture

### Frontend (Desktop Shell & UI)
- **Framework:** Tauri v2
- **UI Layer:** React 19 / TypeScript / Vite
- **Styling & Components:** Tailwind CSS, Lucide Icons, Radix UI primitives
- **State Management:** Zustand (for multi-step wizard state and live queue updates)

### Backend (System Operations & Core Pipeline)
- **Language:** Rust
- **Async Runtime:** `tokio` (for multi-threaded file walking, hashing, and subprocess streaming)
- **Subprocess Management:** Bundled `chdman` binary sidecar + PATH fallback
- **HTTP Client:** `reqwest` for TypeSafe Jev API System One calls
- **Safety & File Operations:** `trash` crate (moving to OS Recycle Bin instead of hard delete)

```
+-----------------------------------------------------------------------+
|                      Tauri Frontend (React + TypeScript)              |
|  [ Config / Preset ] -> [ Scan & Dry-Run Preview ] -> [ Live Progress ]|
+-----------------------------------^-----------------------------------+
                                    | Tauri IPC (Commands & Events)
+-----------------------------------v-----------------------------------+
|                              Rust Backend                             |
|                                                                       |
|  +----------------+     +-------------------+     +----------------+  |
|  | File Scanner   | --> | Classifier Engine | --> | Plan Builder   |  |
|  | (.cue/.bin/.iso|     | (Hash + Jev API)  |     | (IngestionPlan)|  |
|  +----------------+     +-------------------+     +-------+--------+  |
|                                                           |           |
|                                                           v           |
|  +----------------+     +-------------------+     +----------------+  |
|  | Verifier &     | <-- | chdman Runner     | <-- | Job Queue      |  |
|  | M3U Generator  |     | (Stdout Streaming)|     | (Bounded Pool) |  |
|  +----------------+     +-------------------+     +----------------+  |
+-----------------------------------------------------------------------+
```

---

## 3. Data Model & Type Definitions

### 3.1 `DiscFingerprint`
Represents an uncompressed disc entity found during directory scanning:
```rust
pub struct DiscFingerprint {
    pub primary_file: PathBuf,          // The .cue, .gdi, or standalone .iso
    pub binary_tracks: Vec<PathBuf>,    // Associated .bin / .raw track files
    pub detected_platform: Platform,    // PSX, Saturn, Dreamcast, SegaCD, PCECD, Unknown
    pub calculated_sha1: Option<String>,// Partial/First-track SHA-1
    pub total_bytes: u64,
}
```

### 3.2 `GameClassification`
Metadata resolved via Redump lookup or TypeSafe Jev System One:
```rust
pub struct GameClassification {
    pub canonical_title: String,
    pub platform: Platform,
    pub region: String,                 // USA, EUR, JPN, WORLD, UNKNOWN
    pub is_multidisc: bool,
    pub disc_number: Option<u8>,
    pub total_discs: Option<u8>,
    pub confidence: f32,                // 0.0 - 1.0 (calibrated from Jev)
    pub source: ClassificationSource,   // RedumpCache | JevAI | Fallback
}
```

### 3.3 `IngestionPlan` & `PlannedGame`
The immutable plan generated during the dry-run phase:
```rust
pub struct IngestionPlan {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub preset: FrontendPreset,
    pub games: Vec<PlannedGame>,
    pub total_source_bytes: u64,
    pub estimated_output_bytes: u64,
}

pub struct PlannedGame {
    pub id: String,
    pub canonical_title: String,
    pub platform: Platform,
    pub is_multidisc: bool,
    pub discs: Vec<PlannedDisc>,
    pub target_m3u_path: Option<PathBuf>,
    pub enabled: bool,
    pub needs_review: bool,
}

pub struct PlannedDisc {
    pub disc_number: u8,
    pub source_descriptor: PathBuf,
    pub target_chd_path: PathBuf,
    pub status: TaskStatus, // Pending | Compressing | Verified | Failed
}
```

---

## 4. Subsystems & Execution Pipeline

### 4.1 Discovery & Pairing Engine
1. Recursively traverses the input directory.
2. Identifies primary disc descriptors (`.cue`, `.gdi`, `.toc`, `.iso`).
3. Parses descriptor text to resolve referenced binary files:
   - For `.cue`: Reads `FILE "..." BINARY/MOTOROLA` statements and maps each referenced `.bin` file relative to the `.cue` path.
   - For `.gdi`: Reads track count and filename list for Dreamcast GD-ROMs.
   - For orphaned `.bin` files lacking a descriptor: Checks for standard 2352-byte/sector sync headers (`00 FF FF FF FF FF FF FF FF FF FF 00`) and generates an in-memory synthetic `.cue`.

### 4.2 Classification Engine (Redump + TypeSafe Jev System One)
1. **Tier 1: Redump Hash Cache:**
   - Computes SHA-1 on the initial 16MB of Track 1 (or entire Track 1 if <16MB) to match against an embedded SQLite/binary lookup table of known Redump verified dumps.
   - If matched, returns exact `canonical_title`, `region`, and disc numbers with `confidence = 1.0`.
2. **Tier 2: TypeSafe Jev System One (`jev-latest`):**
   - For unmatched files, constructs a structured evaluation request sent to `POST https://api.typesafe.ai/v1/systemone`.
   - Sends file attributes in `state` (clean filename, folder context, track count).
   - Evaluates parallel typed questions:
     - `platform` (`choice`): Resolves retro platform.
     - `is_multidisc` (`noul`): Returns probability that the file is part of a multi-disc game.
     - `disc_number` (`choice`): Maps to `1..8` or `single`.
     - `region` (`choice`): Evaluates target release region.
     - `canonical_match` (`choice`): Compares clean title stem against candidate titles from a normalized title directory.
3. **Confidence Routing:**
   - If `confidence >= 0.80`, automatically accepts classification.
   - If `confidence < 0.80`, flags `needs_review = true` to highlight the item in the UI for user review.

### 4.3 Interactive Dry-Run Review (UI)
Before modifying the disk, the frontend presents the full plan:
- Summary metrics: total games found, multi-disc sets identified, estimated SD card savings.
- Interactive table:
  - Checkbox to toggle inclusion.
  - Inline title editor if user wishes to adjust canonical naming.
  - Confidence badges (`High 96%` vs `Review Needed 58%`).
  - Disc grouping overview (e.g. Discs 1, 2, 3 grouped under single `.m3u`).

### 4.4 `chdman` Runner & Concurrency Queue
- Spawns `chdman createcd -i "<source_descriptor>" -o "<target_chd_path>.part" -f`.
- Employs a bounded task pool (default: `min(physical_cores, 3)` concurrent processes) to prevent CPU and I/O starvation.
- Asynchronously parses stdout line-by-line:
  - Extracts progress percent (e.g., `Compressing, 43.1% complete...`) and streams `JobProgressEvent` to the UI.
- On exit code `0`:
  - Validates CHD v5 header magic bytes (`MComprHD`).
  - Renames `<target_chd_path>.part` to `<target_chd_path>`.
- On non-zero exit or abort:
  - Immediately deletes partial `.part` files.
  - Marks disc as `Failed` with error message, keeping remaining queue intact.

### 4.5 M3U Generation & Frontend Presets
Once all discs for a game are verified:
- Single-disc games are placed directly in the system ROM folder.
- Multi-disc games are placed in a hidden `.discs/` subfolder.
- The `.m3u` file is written in UTF-8 with relative paths:
  ```text
  .discs/Metal Gear Solid (USA) (Disc 1).chd
  .discs/Metal Gear Solid (USA) (Disc 2).chd
  ```

#### Built-in Preset Mappings:
| Preset | Platform Subfolders | Multi-Disc Subfolder | Notes |
| :--- | :--- | :--- | :--- |
| **ES-DE** | `roms/psx/`, `roms/saturn/`, `roms/dreamcast/`, `roms/segacd/`, `roms/pcenginecd/` | `.discs/` | Hidden dot folder prevents scraper duplicate entries |
| **OnionOS / GarlicOS** | `Roms/PS/`, `Roms/SEGASATURN/`, `Roms/DREAMCAST/`, `Roms/SEGACD/`, `Roms/PCECD/` | `.discs/` | Strict handheld naming convention |
| **Anbernic Stock OS** | `ROMS/PS/`, `ROMS/SATURN/`, `ROMS/DC/`, `ROMS/MDCD/`, `ROMS/PCE/` | `.discs/` | Standard FAT32/exFAT uppercase folder convention |
| **Batocera / Knulli** | `roms/psx/`, `roms/saturn/`, `roms/dreamcast/`, etc. | `.discs/` | Direct RetroArch/ES integration |
| **Custom / Flat** | User-configured | Configurable | Allows custom output paths |

### 4.6 Safety, Verification & Cleanup
- **Non-Destructive Guarantee:** Input files are opened read-only. Original files are never modified in-place.
- **Optional Cleanup:**
  - "Move source dumps to Trash" or "Archive original files" toggle.
  - Only accessible after 100% of planned games pass CHD verification.
  - Uses the system Recycle Bin (`trash` crate) rather than permanent immediate deletion.
  - Requires explicit secondary user confirmation.

---

## 5. Verification & Testing Plan

### 5.1 Unit Tests (Rust)
- **CUE Parser:** Test single `.bin`, multi-track `.bin`, and relative subfolder paths.
- **M3U Generator:** Test relative path resolution and special character preservation.
- **TypeSafe Jev Serialization:** Test JSON payload generation and typed response parsing for `Choice` and `Noul` primitives.

### 5.2 Integration Tests
- Mock `chdman` subprocess harness verifying stdout progress regex parsing, exit code handling, and cleanup on failure.
- End-to-end dry run on mock multi-disc directory fixture.

### 5.3 UI / Component Tests (Vitest + React Testing Library)
- Wizard step transitions (`Setup` $\rightarrow$ `Plan Review` $\rightarrow$ `Execution` $\rightarrow$ `Summary`).
- Ingestion plan table sorting, title overrides, and confidence badges.
