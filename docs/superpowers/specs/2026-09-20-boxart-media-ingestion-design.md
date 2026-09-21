# Built-in Box Art & Media Ingestion Engine - Architecture & Design Specification

**Date:** 2026-09-20  
**Status:** Approved  
**Target Platform:** Desktop (Tauri v2: Rust + React 19 / TypeScript / Tailwind CSS)

---

## 1. Overview & Problem Statement

Retro handheld frontends (such as **Anbernic Stock OS**, **OnionOS/GarlicOS**, **ES-DE**, and **Batocera/Knulli**) require properly structured and named image files in specific directories to render box art and screenshots. Scraping media directly on handheld devices over Wi-Fi is notoriously slow, battery-draining, and prone to scraping mismatched titles.

This specification introduces the **Built-in Box Art & Media Ingestion Engine**:
1. Uses the **Libretro Open Thumbnails Database** as a free, open-source, zero-configuration CDN source for front box art, screenshots, and title screens.
2. Resolves media URLs during the dry-run planning phase, providing visual thumbnail previews in the Step 2 Review Table.
3. Automatically maps target media folders according to the active frontend preset (e.g. `ROMS/PS/Imgs/<Title>.png` for Anbernic Stock OS, `media/covers/` for ES-DE).
4. Strictly matches multi-disc artwork to the root `.m3u` playlist filename stem, ensuring frontends display rich artwork on the single playlist item.
5. Asynchronously downloads and verifies images during plan execution without blocking CPU-intensive `chdman` compression.

---

## 2. Libretro Taxonomy & CDN URL Resolution

### 2.1 Platform Taxonomy Mapping
In `src-tauri/src/organizer/media.rs`:

```rust
pub fn platform_to_libretro_system(platform: Platform) -> Option<&'static str> {
    match platform {
        Platform::Psx => Some("Sony - PlayStation"),
        Platform::Saturn => Some("Sega - Saturn"),
        Platform::Dreamcast => Some("Sega - Dreamcast"),
        Platform::SegaCd => Some("Sega - Mega-CD - Sega CD"),
        Platform::PceCd => Some("NEC - PC Engine CD - TurboGrafx-CD"),
        Platform::Unknown => None,
    }
}
```

### 2.2 CDN URL Pattern & Sanitation
Base CDN format:
`https://raw.githubusercontent.com/libretro-thumbnails/{System}/master/{MediaType}/{SanitizedTitle}.png`

Supported media types:
- **`MediaType::BoxArt`**: `Named_Boxarts`
- **`MediaType::Screenshots`**: `Named_Snaps`
- **`MediaType::TitleScreens`**: `Named_Titles`

**Sanitization Rules:**
Characters invalid in Libretro file repositories (`&`, `*`, `/`, `:`, `\`, `<`, `>`, `?`, `|`) are converted to `_`.

**Candidate Fallback Order:**
1. `"{CanonicalTitle} ({Region})"` (Exact match, e.g. `Final Fantasy VII (USA)`)
2. `"{CanonicalTitle}"` (Title only without region tag)
3. Clean title stem (stripping subtitles or punctuation like `:`)

---

## 3. Preset Media Directory Mappings

| Preset | Target Media Directory | Target File Naming |
| :--- | :--- | :--- |
| **Anbernic Stock OS** | `ROMS/<SYSTEM>/Imgs/` | `<Stem>.png` |
| **OnionOS / GarlicOS** | `Roms/<SYSTEM>/Imgs/` | `<Stem>.png` |
| **ES-DE** | `roms/<system>/media/covers/` | `<Stem>.png` |
| **Batocera / Knulli** | `roms/<system>/images/` | `<Stem>-thumb.png` |
| **Custom** | `custom/path/images/` | Configurable |

*Where `<Stem>` is the `.m3u` file stem for multi-disc sets, or the `.chd` file stem for single-disc games.*

---

## 4. Data Models & IPC Interfaces

### 4.1 Data Models
In `src-tauri/src/models.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    BoxArt,
    Screenshots,
    TitleScreens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaOptions {
    pub download_boxart: bool,
    pub download_screenshots: bool,
    pub download_titles: bool,
}

impl Default for MediaOptions {
    fn default() -> Self {
        Self {
            download_boxart: true,
            download_screenshots: false,
            download_titles: false,
        }
    }
}

// In PlannedGame:
pub struct PlannedGame {
    // ... existing fields ...
    pub artwork_url: Option<String>,
    pub target_media_paths: Vec<PathBuf>,
}
```

### 4.2 IPC Commands & Operations
- `resolve_game_artwork(platform: Platform, canonical_title: String, region: String) -> Result<Option<String>, String>`: Tests and returns the verified CDN thumbnail URL.
- Updated `execute_plan`: In addition to converting discs via `chdman`, downloads verified media assets concurrently and saves them to the preset's target media folders.

---

## 5. UI Integration (Steps 1 & 2)

### 5.1 Step 1: Media Selection Controls
A new card in `Step1Config.tsx`:
- `[✓] Download Front Box Art (Default)`
- `[ ] Download Gameplay Screenshots`
- `[ ] Download Title Screens`

### 5.2 Step 2: Interactive Artwork Column
In `Step2DryRunTable.tsx`:
- New **Artwork** column rendering an 80px high-resolution preview image.
- Image includes hover-zoom card.
- If no asset is found, displays a `No Artwork` badge with an inline search box allowing the user to search an alternative title.

---

## 6. Verification & Test Plan

### 6.1 Unit Tests (Rust)
- Libretro system repository mapping for all 5 platforms.
- Libretro title sanitization (character escaping).
- Target media path resolution for each frontend preset (Anbernic Stock, OnionOS, ES-DE, Batocera).
- Multi-disc artwork stem resolution (matches `.m3u` name).

### 6.2 Integration Tests
- Mock HTTP HEAD test verifying asset discovery and fallback resolution.
- End-to-end plan execution test verifying artwork `.png` is written to `ROMS/PS/Imgs/` alongside `.m3u` and `.chd` files.

### 6.3 Frontend Component Tests (Vitest)
- Test media option toggles in `Step1Config`.
- Test artwork thumbnail rendering and fallback placeholder in `Step2DryRunTable`.
