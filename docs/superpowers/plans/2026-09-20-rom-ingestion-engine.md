# Intelligent ROM Library Ingestion Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform Tauri v2 desktop application that automates the ingestion, Redump/TypeSafe Jev AI classification, chdman compression, and frontend-ready (.m3u and hidden discs) organization of retro disc-based ROM collections.

**Architecture:** Tauri v2 desktop application with a modular Rust core backend and a React/TypeScript frontend. The backend implements a staged pipeline: directory scanner & CUE parser -> two-tier classifier (local Redump SHA-1 cache + TypeSafe Jev System One API) -> immutable `IngestionPlan` builder -> bounded `chdman` worker queue with real-time progress streaming -> M3U playlist and frontend preset organizer with safe OS trash cleanup.

**Tech Stack:** Rust (tokio, serde, reqwest, regex, sha1, trash), Tauri v2, React 19, TypeScript, Vite, Tailwind CSS, Lucide React.

## Global Constraints
- Target disc platforms in v1: Sony PlayStation (PSX), Sega Saturn, Sega Dreamcast, Sega CD, PC Engine CD.
- Default to non-destructive processing: original dumps are never modified in-place.
- Use OS Recycle Bin / Trash (`trash` crate) rather than immediate `unlink` for source cleanup, gated behind 100% verification and explicit user confirmation.
- Built-in frontend presets must include: ES-DE, OnionOS/GarlicOS, Anbernic Stock OS, Batocera/Knulli, and Custom.
- Model identifier for TypeSafe Jev System One calls is `jev-latest`.

---

### Task 1: Scaffolding Tauri v2 Project & Core Data Models

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/models_test.rs`

**Interfaces:**
- Produces: `Platform`, `DiscFingerprint`, `GameClassification`, `ClassificationSource`, `FrontendPreset`, `IngestionPlan`, `PlannedGame`, `PlannedDisc`, `TaskStatus`.

- [ ] **Step 1: Write failing unit test for core models serialization**

```rust
// src-tauri/tests/models_test.rs
use std::path::PathBuf;

#[test]
fn test_models_json_roundtrip() {
    use rom_ingest_core::models::*;

    let disc = PlannedDisc {
        disc_number: 1,
        source_descriptor: PathBuf::from("C:/Roms/FF7_Disc1.cue"),
        target_chd_path: PathBuf::from("C:/Output/psx/.discs/Final Fantasy VII (USA) (Disc 1).chd"),
        status: TaskStatus::Pending,
    };

    let game = PlannedGame {
        id: "game-1".to_string(),
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        discs: vec![disc],
        target_m3u_path: Some(PathBuf::from("C:/Output/psx/Final Fantasy VII (USA).m3u")),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
        enabled: true,
        needs_review: false,
    };

    let plan = IngestionPlan {
        input_dir: PathBuf::from("C:/Roms"),
        output_dir: PathBuf::from("C:/Output"),
        preset: FrontendPreset::AnbernicStock,
        games: vec![game],
        total_source_bytes: 700_000_000,
        estimated_output_bytes: 450_000_000,
    };

    let serialized = serde_json::to_string(&plan).expect("Failed to serialize");
    let deserialized: IngestionPlan = serde_json::from_str(&serialized).expect("Failed to deserialize");
    assert_eq!(deserialized.games[0].canonical_title, "Final Fantasy VII");
    assert_eq!(deserialized.preset, FrontendPreset::AnbernicStock);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test models_test` from workspace
Expected: FAIL due to missing files/modules.

- [ ] **Step 3: Implement `src-tauri/Cargo.toml` and `src-tauri/src/models.rs`**

```rust
// src-tauri/src/models.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Psx,
    Saturn,
    Dreamcast,
    SegaCd,
    PceCd,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscFingerprint {
    pub primary_file: PathBuf,
    pub binary_tracks: Vec<PathBuf>,
    pub detected_platform: Platform,
    pub calculated_sha1: Option<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassificationSource {
    RedumpCache,
    JevAI,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameClassification {
    pub canonical_title: String,
    pub platform: Platform,
    pub region: String,
    pub is_multidisc: bool,
    pub disc_number: Option<u8>,
    pub total_discs: Option<u8>,
    pub confidence: f32,
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontendPreset {
    EsDe,
    OnionOs,
    AnbernicStock,
    Batocera,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Compressing,
    Verified,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedDisc {
    pub disc_number: u8,
    pub source_descriptor: PathBuf,
    pub target_chd_path: PathBuf,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedGame {
    pub id: String,
    pub canonical_title: String,
    pub platform: Platform,
    pub region: String,
    pub is_multidisc: bool,
    pub discs: Vec<PlannedDisc>,
    pub target_m3u_path: Option<PathBuf>,
    pub confidence: f32,
    pub source: ClassificationSource,
    pub enabled: bool,
    pub needs_review: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionPlan {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub preset: FrontendPreset,
    pub games: Vec<PlannedGame>,
    pub total_source_bytes: u64,
    pub estimated_output_bytes: u64,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test models_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat(core): scaffold tauri project and core data models"
```

---

### Task 2: CUE / GDI Parser & Multi-Track File Scanner

**Files:**
- Create: `src-tauri/src/scanner/cue_parser.rs`
- Create: `src-tauri/src/scanner/mod.rs`
- Test: `src-tauri/tests/scanner_test.rs`

**Interfaces:**
- Consumes: `DiscFingerprint`, `Platform` from `models.rs`
- Produces: `CueSheet`, `parse_cue_content(content: &str, base_dir: &Path) -> Result<Vec<PathBuf>, ScannerError>`, `scan_directory(root: &Path) -> Result<Vec<DiscFingerprint>, ScannerError>`.

- [ ] **Step 1: Write failing test for CUE sheet parsing**

```rust
// src-tauri/tests/scanner_test.rs
use std::path::Path;
use rom_ingest_core::scanner::cue_parser::parse_cue_references;

#[test]
fn test_parse_cue_single_and_multitrack() {
    let single_track_cue = r#"
FILE "Game (USA).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
"#;
    let refs = parse_cue_references(single_track_cue);
    assert_eq!(refs, vec!["Game (USA).bin"]);

    let multi_track_cue = r#"
FILE "Ridge Racer (USA) (Track 1).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
FILE "Ridge Racer (USA) (Track 2).bin" BINARY
  TRACK 02 AUDIO
    INDEX 00 00:00:00
    INDEX 01 00:02:00
"#;
    let refs = parse_cue_references(multi_track_cue);
    assert_eq!(refs, vec!["Ridge Racer (USA) (Track 1).bin", "Ridge Racer (USA) (Track 2).bin"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scanner_test`
Expected: FAIL

- [ ] **Step 3: Implement `cue_parser.rs` and recursive directory scanner**

```rust
// src-tauri/src/scanner/cue_parser.rs
use regex::Regex;

pub fn parse_cue_references(content: &str) -> Vec<String> {
    let re = Regex::new(r#"(?i)FILE\s+["']([^"']+)["']"#).unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}
```

Implement `src-tauri/src/scanner/mod.rs` to traverse directories, discover `.cue`, `.gdi`, and standalone `.iso` files, match them with associated binary tracks, calculate file size and first-track partial SHA-1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scanner_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scanner/ src-tauri/tests/scanner_test.rs
git commit -m "feat(scanner): implement cue parser and multi-track disc scanner"
```

---

### Task 3: Redump Local Hash Database & Cache Lookup

**Files:**
- Create: `src-tauri/src/classifier/redump.rs`
- Test: `src-tauri/tests/redump_test.rs`

**Interfaces:**
- Consumes: `DiscFingerprint`
- Produces: `RedumpDatabase::lookup_sha1(&self, sha1: &str) -> Option<GameClassification>`

- [ ] **Step 1: Write failing test for Redump cache lookup**

```rust
// src-tauri/tests/redump_test.rs
use rom_ingest_core::classifier::redump::RedumpDatabase;
use rom_ingest_core::models::Platform;

#[test]
fn test_redump_lookup_known_hash() {
    let db = RedumpDatabase::new_in_memory();
    db.insert("a1b2c3d4e5f67890123456789abcdef012345678", "Metal Gear Solid", Platform::Psx, "USA", true, Some(1), Some(2));

    let result = db.lookup_sha1("a1b2c3d4e5f67890123456789abcdef012345678");
    assert!(result.is_some());
    let info = result.unwrap();
    assert_eq!(info.canonical_title, "Metal Gear Solid");
    assert_eq!(info.disc_number, Some(1));
    assert_eq!(info.confidence, 1.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test redump_test`
Expected: FAIL

- [ ] **Step 3: Implement `RedumpDatabase` with fast hash lookup**

Implement in-memory hash index with SQLite backing or pre-indexed binary hash map for known Redump releases.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test redump_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/classifier/redump.rs src-tauri/tests/redump_test.rs
git commit -m "feat(classifier): implement redump local hash cache lookup"
```

---

### Task 4: TypeSafe Jev System One Client & Question Evaluator

**Files:**
- Create: `src-tauri/src/classifier/jev.rs`
- Test: `src-tauri/tests/jev_test.rs`

**Interfaces:**
- Consumes: Raw filename, platform hints, and API key
- Produces: `JevClient::evaluate_game_filename(filename: &str, folder: &str) -> Result<GameClassification, JevError>` using TypeSafe System One (`jev-latest`).

- [ ] **Step 1: Write failing test for Jev request/response serialization**

```rust
// src-tauri/tests/jev_test.rs
use rom_ingest_core::classifier::jev::*;

#[test]
fn test_jev_request_building_and_response_parsing() {
    let payload = build_jev_request("Final Fantasy VII (USA) (Disc 2).cue", "PSX");
    assert_eq!(payload.model, "jev-latest");
    assert!(payload.questions.contains_key("is_multidisc"));
    assert!(payload.questions.contains_key("disc_number"));

    let mock_response_json = r#"{
      "model": "jev-1.13.0",
      "answers": {
        "platform": {
          "type": "choice",
          "choice": "psx",
          "probabilities": { "psx": 0.99, "saturn": 0.01 },
          "confidence": 0.98
        },
        "is_multidisc": {
          "type": "noul",
          "noul": 0.99
        },
        "disc_number": {
          "type": "choice",
          "choice": "2",
          "probabilities": { "2": 0.97, "1": 0.03 },
          "confidence": 0.95
        },
        "region": {
          "type": "choice",
          "choice": "USA",
          "probabilities": { "USA": 0.99 },
          "confidence": 0.99
        }
      },
      "usage": { "input_tokens": 250, "output_tokens": 30 }
    }"#;

    let classification = parse_jev_response(mock_response_json, "Final Fantasy VII (USA) (Disc 2).cue").unwrap();
    assert_eq!(classification.disc_number, Some(2));
    assert!(classification.is_multidisc);
    assert_eq!(classification.region, "USA");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test jev_test`
Expected: FAIL

- [ ] **Step 3: Implement `src-tauri/src/classifier/jev.rs`**

Implement HTTP call to `https://api.typesafe.ai/v1/systemone` using `reqwest` with Bearer auth, exponential backoff, and JSON deserialization of answers.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test jev_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/classifier/jev.rs src-tauri/tests/jev_test.rs
git commit -m "feat(classifier): implement typesafe jev system one evaluator"
```

---

### Task 5: Frontend Presets & M3U Playlist Generator

**Files:**
- Create: `src-tauri/src/organizer/presets.rs`
- Create: `src-tauri/src/organizer/m3u.rs`
- Test: `src-tauri/tests/organizer_test.rs`

**Interfaces:**
- Consumes: `FrontendPreset`, `Platform`, `PlannedGame`
- Produces: `resolve_target_paths(preset: FrontendPreset, game: &GameClassification) -> (PathBuf, Option<PathBuf>)`, `generate_m3u_content(disc_rel_paths: &[&str]) -> String`.

- [ ] **Step 1: Write failing test for preset path resolution & M3U generation**

```rust
// src-tauri/tests/organizer_test.rs
use std::path::PathBuf;
use rom_ingest_core::models::*;
use rom_ingest_core::organizer::presets::*;
use rom_ingest_core::organizer::m3u::*;

#[test]
fn test_m3u_formatting() {
    let paths = vec![
        ".discs/Final Fantasy VII (USA) (Disc 1).chd",
        ".discs/Final Fantasy VII (USA) (Disc 2).chd",
        ".discs/Final Fantasy VII (USA) (Disc 3).chd",
    ];
    let content = generate_m3u_content(&paths);
    assert_eq!(
        content,
        ".discs/Final Fantasy VII (USA) (Disc 1).chd\n.discs/Final Fantasy VII (USA) (Disc 2).chd\n.discs/Final Fantasy VII (USA) (Disc 3).chd\n"
    );
}

#[test]
fn test_anbernic_stock_preset_paths() {
    let base = PathBuf::from("E:/");
    let rel_dir = get_platform_folder(FrontendPreset::AnbernicStock, Platform::Psx);
    assert_eq!(rel_dir, "ROMS/PS");

    let saturn_dir = get_platform_folder(FrontendPreset::AnbernicStock, Platform::Saturn);
    assert_eq!(saturn_dir, "ROMS/SATURN");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test organizer_test`
Expected: FAIL

- [ ] **Step 3: Implement `presets.rs` and `m3u.rs`**

Implement mappings for `ES-DE`, `OnionOS`, `AnbernicStock`, `Batocera`, and `Custom`, along with safe relative path construction for `.m3u` playlists.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test organizer_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/organizer/ src-tauri/tests/organizer_test.rs
git commit -m "feat(organizer): implement frontend presets and m3u generator"
```

---

### Task 6: `chdman` Subprocess Runner & Progress Streamer

**Files:**
- Create: `src-tauri/src/chdman/runner.rs`
- Create: `src-tauri/src/chdman/mod.rs`
- Test: `src-tauri/tests/chdman_test.rs`

**Interfaces:**
- Consumes: Input file, output file, cancel channel
- Produces: `execute_chd_conversion<F>(input: &Path, output: &Path, on_progress: F) -> Result<(), ChdmanError>` where `on_progress: Fn(f32)`.

- [ ] **Step 1: Write failing test for chdman stdout progress regex parsing**

```rust
// src-tauri/tests/chdman_test.rs
use rom_ingest_core::chdman::runner::parse_chdman_progress_line;

#[test]
fn test_parse_chdman_progress() {
    assert_eq!(parse_chdman_progress_line("Compressing, 42.8% complete..."), Some(42.8));
    assert_eq!(parse_chdman_progress_line("Compressing, 100.0% complete..."), Some(100.0));
    assert_eq!(parse_chdman_progress_line("chdman - MAME Compressed Hunks of Data (CHD) manager 0.268"), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test chdman_test`
Expected: FAIL

- [ ] **Step 3: Implement `src-tauri/src/chdman/runner.rs`**

Spawns `chdman createcd -i "<input>" -o "<output>.part" -f`, parses stdout asynchronously, emits progress callbacks, validates header on exit code 0, renames from `.part` to `.chd`, and cleans up on error.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test chdman_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/chdman/ src-tauri/tests/chdman_test.rs
git commit -m "feat(chdman): implement subprocess runner and progress parser"
```

---

### Task 7: Ingestion Plan Builder & Tauri IPC Commands

**Files:**
- Create: `src-tauri/src/plan_builder.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/plan_builder_test.rs`

**Interfaces:**
- Consumes: `Vec<DiscFingerprint>`, `Vec<GameClassification>`, `FrontendPreset`
- Produces: `build_ingestion_plan(...) -> IngestionPlan`, Tauri commands `scan_and_plan`, `execute_plan`, `trash_source_files`.

- [ ] **Step 1: Write failing test for multi-disc plan grouping**

```rust
// src-tauri/tests/plan_builder_test.rs
use std::path::PathBuf;
use rom_ingest_core::models::*;
use rom_ingest_core::plan_builder::build_ingestion_plan;

#[test]
fn test_multi_disc_plan_grouping() {
    // Two discs of same title must merge into 1 PlannedGame with 2 PlannedDiscs
    let disc1 = DiscFingerprint {
        primary_file: PathBuf::from("in/FF7_1.cue"),
        binary_tracks: vec![PathBuf::from("in/FF7_1.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 700_000_000,
    };
    let disc2 = DiscFingerprint {
        primary_file: PathBuf::from("in/FF7_2.cue"),
        binary_tracks: vec![PathBuf::from("in/FF7_2.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 700_000_000,
    };

    let class1 = GameClassification {
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(1),
        total_discs: Some(3),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
    };
    let class2 = GameClassification {
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(2),
        total_discs: Some(3),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::AnbernicStock,
        vec![(disc1, class1), (disc2, class2)],
    );

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].discs.len(), 2);
    assert!(plan.games[0].target_m3u_path.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plan_builder_test`
Expected: FAIL

- [ ] **Step 3: Implement plan builder and Tauri IPC commands**

Implement `build_ingestion_plan` grouping algorithm and wire Tauri commands in `commands.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test plan_builder_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/plan_builder.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/tests/plan_builder_test.rs
git commit -m "feat(plan): implement plan builder and tauri commands"
```

---

### Task 8: Frontend UI (React + Tailwind + Zustand)

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `tailwind.config.js`
- Create: `src/App.tsx`
- Create: `src/store/useIngestionStore.ts`
- Create: `src/components/Step1Config.tsx`
- Create: `src/components/Step2DryRunTable.tsx`
- Create: `src/components/Step3ExecutionProgress.tsx`
- Create: `src/components/Step4Summary.tsx`
- Test: `src/components/__tests__/DryRunTable.test.tsx`

**Interfaces:**
- Consumes: Tauri IPC invokes (`scan_and_plan`, `execute_plan`, `trash_source_files`), Tauri event streams (`job-progress`, `game-status`).
- Produces: 4-step wizard desktop UI.

- [ ] **Step 1: Write failing frontend test for DryRunTable component**

```tsx
// src/components/__tests__/DryRunTable.test.tsx
import { render, screen } from '@testing-library/react';
import { DryRunTable } from '../Step2DryRunTable';
import { PlannedGame } from '../../types/plan';

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
    source: 'jev_ai',
    enabled: true,
    needs_review: false,
  }
];

test('renders game title, badges, and disc count', () => {
  render(<DryRunTable games={mockGames} onToggle={() => {}} onUpdateTitle={() => {}} />);
  expect(screen.getByText('Final Fantasy VII')).toBeInTheDocument();
  expect(screen.getByText(/2 Discs/i)).toBeInTheDocument();
  expect(screen.getByText(/95%/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run frontend test to verify it fails**

Run: `npm test`
Expected: FAIL

- [ ] **Step 3: Implement frontend components and Zustand store**

Build the modern wizard UI with Tailwind CSS:
- Step 1: Input/Output folder selector, Preset dropdown (ES-DE, OnionOS, Anbernic Stock, Batocera, Custom), Scan button.
- Step 2: Dry Run Review Table with badges, confidence scores, title editing, and exclusion toggles.
- Step 3: Execution progress with overall % and individual worker task bars.
- Step 4: Summary card showing total space saved and optional Trash confirmation modal.

- [ ] **Step 4: Run frontend test to verify it passes**

Run: `npm test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add package.json src/ vite.config.ts tailwind.config.js
git commit -m "feat(ui): implement modern react frontend wizard and plan review table"
```

---

### Task 9: End-to-End Verification & Bundling

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/tests/e2e_pipeline_test.rs`

- [ ] **Step 1: Write integration test exercising full pipeline with mock files**

Create a test folder with a 2-disc dummy `.cue` set, run `scan_and_plan`, verify the resulting `IngestionPlan` contains correct `.m3u` and `.chd` paths according to the Anbernic Stock preset.

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --test e2e_pipeline_test`
Expected: PASS

- [ ] **Step 3: Build frontend and verify desktop bundle compile**

Run: `npm run build && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Success with 0 errors.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(pipeline): verify end-to-end integration and compile health"
```
