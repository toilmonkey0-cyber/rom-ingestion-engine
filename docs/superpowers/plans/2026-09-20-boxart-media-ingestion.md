# Built-in Box Art & Media Ingestion Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement automatic box art, screenshot, and title screen scraping from Libretro Open Thumbnails with interactive dry-run table previews and frontend-specific media directory organization.

**Architecture:** A Rust module (`src-tauri/src/organizer/media.rs`) maps platforms to Libretro repositories, generates sanitized candidate URLs, performs asynchronous `HEAD` checks to resolve working assets, and downloads `.png` media files directly to the preset's required media folder (e.g. `ROMS/PS/Imgs/<Game>.png` for Anbernic Stock OS, `media/covers/` for ES-DE). The React frontend provides media option toggles in Step 1 and renders real-time artwork preview thumbnails in the Step 2 review table.

**Tech Stack:** Rust (reqwest, serde, tokio), Tauri v2, React 19, TypeScript, Tailwind CSS, Lucide React, Vitest.

## Global Constraints
- Target disc platforms in v1: Sony PlayStation (PSX), Sega Saturn, Sega Dreamcast, Sega CD, PC Engine CD.
- Free, zero-configuration: use Libretro Open Thumbnails CDN (no accounts or API keys required).
- Non-destructive processing: original dumps remain untouched.
- Multi-disc artwork must match the root `.m3u` playlist filename stem so handheld frontends display cover art on the playlist entry.
- Built-in frontend presets must route media correctly:
  - **Anbernic Stock OS:** `ROMS/<SYSTEM>/Imgs/<Stem>.png`
  - **OnionOS / GarlicOS:** `Roms/<SYSTEM>/Imgs/<Stem>.png`
  - **ES-DE:** `roms/<system>/media/covers/<Stem>.png` (and `screenshots/`)
  - **Batocera / Knulli:** `roms/<system>/images/<Stem>-thumb.png`
  - **Custom:** configurable

---

### Task 1: Core Media Models, Libretro URL Resolver & Preset Path Mappings

**Files:**
- Modify: `src-tauri/src/models.rs`
- Create: `src-tauri/src/organizer/media.rs`
- Modify: `src-tauri/src/organizer/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/media_test.rs`

**Interfaces:**
- Produces:
  - `MediaType` (BoxArt, Screenshots, TitleScreens)
  - `MediaOptions` (`download_boxart: bool`, `download_screenshots: bool`, `download_titles: bool`)
  - `PlannedGame` updated with `pub artwork_url: Option<String>` and `pub target_media_paths: Vec<PathBuf>`
  - `platform_to_libretro_system(platform: Platform) -> Option<&'static str>`
  - `sanitize_libretro_title(title: &str) -> String`
  - `generate_candidate_urls(platform: Platform, title: &str, region: &str, media_type: MediaType) -> Vec<String>`
  - `resolve_preset_media_path(output_dir: &Path, preset: FrontendPreset, platform: Platform, stem: &str, media_type: MediaType) -> PathBuf`

- [ ] **Step 1: Write failing test for Libretro URL generation and preset path resolution**

```rust
// src-tauri/tests/media_test.rs
use std::path::PathBuf;
use rom_ingest_core::models::{FrontendPreset, Platform};
use rom_ingest_core::organizer::media::*;

#[test]
fn test_libretro_system_mapping() {
    assert_eq!(platform_to_libretro_system(Platform::Psx), Some("Sony - PlayStation"));
    assert_eq!(platform_to_libretro_system(Platform::Saturn), Some("Sega - Saturn"));
    assert_eq!(platform_to_libretro_system(Platform::Dreamcast), Some("Sega - Dreamcast"));
    assert_eq!(platform_to_libretro_system(Platform::SegaCd), Some("Sega - Mega-CD - Sega CD"));
    assert_eq!(platform_to_libretro_system(Platform::PceCd), Some("NEC - PC Engine CD - TurboGrafx-CD"));
}

#[test]
fn test_libretro_title_sanitization() {
    assert_eq!(sanitize_libretro_title("Castlevania: Symphony of the Night"), "Castlevania_ Symphony of the Night");
    assert_eq!(sanitize_libretro_title("Sonic CD & Knuckles?"), "Sonic CD _ Knuckles_");
}

#[test]
fn test_preset_media_path_resolution() {
    let out = PathBuf::from("E:/");
    let anbernic_path = resolve_preset_media_path(
        &out,
        FrontendPreset::AnbernicStock,
        Platform::Psx,
        "Metal Gear Solid (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(anbernic_path, PathBuf::from("E:/ROMS/PS/Imgs/Metal Gear Solid (USA).png"));

    let esde_path = resolve_preset_media_path(
        &out,
        FrontendPreset::EsDe,
        Platform::Psx,
        "Metal Gear Solid (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(esde_path, PathBuf::from("E:/roms/psx/media/covers/Metal Gear Solid (USA).png"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test media_test`
Expected: FAIL

- [ ] **Step 3: Implement `media.rs` models and path mappings**

Implement `platform_to_libretro_system`, `sanitize_libretro_title`, `generate_candidate_urls`, and `resolve_preset_media_path`. Update `models.rs` with `MediaType`, `MediaOptions`, and `PlannedGame` fields.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test media_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat(media): implement libretro url resolver and preset media paths"
```

---

### Task 2: Asynchronous Artwork Verifier, Media Downloader & Tauri IPC

**Files:**
- Modify: `src-tauri/src/organizer/media.rs`
- Modify: `src-tauri/src/plan_builder.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/media_test.rs`
- Test: `src-tauri/tests/commands_test.rs`

**Interfaces:**
- Produces:
  - `async fn resolve_artwork_url(client: &reqwest::Client, platform: Platform, title: &str, region: &str) -> Option<String>`
  - `async fn download_media_file(client: &reqwest::Client, url: &str, dest_path: &Path) -> Result<(), MediaError>`
  - Tauri command: `resolve_game_artwork(platform: Platform, title: String, region: String) -> Result<Option<String>, String>`
  - Updated `scan_and_plan` and `build_ingestion_plan` with `MediaOptions`
  - Updated `execute_plan_internal` to download media assets to target paths on game completion

- [ ] **Step 1: Write failing test for artwork resolution and download with mock HTTP server**

```rust
// In src-tauri/tests/media_test.rs
#[tokio::test]
async fn test_resolve_and_download_artwork_mock_server() {
    // Verifies candidate HEAD checking, image downloading, and saving to disk
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test media_test`
Expected: FAIL

- [ ] **Step 3: Implement async artwork resolver, media downloader, and Tauri commands**

Implement `resolve_artwork_url` with candidate fallback HEAD checking, `download_media_file`, expose `resolve_game_artwork`, and wire media download into `execute_plan_internal`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test media_test && cargo test --test commands_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat(media): implement async artwork resolver and concurrent media downloader"
```

---

### Task 3: Frontend UI Media Controls, Artwork Thumbnail Preview & Tests

**Files:**
- Modify: `src/types/plan.ts`
- Modify: `src/services/tauri.ts`
- Modify: `src/store/useIngestionStore.ts`
- Modify: `src/components/Step1Config.tsx`
- Modify: `src/components/Step2DryRunTable.tsx`
- Modify: `src/components/Step4Summary.tsx`
- Test: `src/components/__tests__/DryRunTable.test.tsx`
- Test: `src/components/__tests__/Step1Config.test.tsx`

**Interfaces:**
- In `types/plan.ts`:
  - `MediaOptions: { download_boxart: boolean; download_screenshots: boolean; download_titles: boolean }`
  - `PlannedGame` carries `artwork_url: string | null` and `target_media_paths: string[]`
- In `Step1Config.tsx`:
  - Media options card allowing user to toggle box art, screenshots, and title screens
- In `Step2DryRunTable.tsx`:
  - Artwork preview column showing 80px thumbnail with hover preview and fallback placeholder
- In `Step4Summary.tsx`:
  - Displays media metrics (e.g. "Cover Art Ingested: 18 / 18 games ready on SD card")

- [ ] **Step 1: Write failing test for artwork preview in DryRunTable**

```tsx
// In src/components/__tests__/DryRunTable.test.tsx
test('renders artwork thumbnail image when artwork_url is present', () => {
  // Test asserting <img> tag with correct src and alt text renders in the table
});
```

- [ ] **Step 2: Run frontend test to verify it fails**

Run: `npm test`
Expected: FAIL

- [ ] **Step 3: Implement frontend UI components and store integration**

Update store with `mediaOptions`, integrate toggles into `Step1Config`, add thumbnail column to `Step2DryRunTable`, and update `Step4Summary`.

- [ ] **Step 4: Run frontend tests to verify they pass**

Run: `npm test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(ui): implement media selection options and interactive artwork previews"
```

---

### Task 4: End-to-End Verification & Health Check

**Files:**
- Modify: `src-tauri/tests/e2e_pipeline_test.rs`
- Run full test suites (`cargo test` & `npm test`)
- Run production build (`npm run build`)

- [ ] **Step 1: Write and run end-to-end integration test verifying media ingestion**

Verify in `e2e_pipeline_test.rs` that when an Anbernic Stock preset plan runs, cover art is fetched and saved to `ROMS/PS/Imgs/Final Fantasy VII (USA).png` matching the `.m3u` playlist.

- [ ] **Step 2: Run all tests and production build**

Run: `cargo test && npm test && npm run build`
Expected: ALL PASS with 0 errors

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "feat(pipeline): verify end-to-end media ingestion and build health"
```
