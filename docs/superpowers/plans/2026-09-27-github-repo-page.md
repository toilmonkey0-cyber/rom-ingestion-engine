# GitHub Repository Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a professional, eye-catching GitHub repository page (`README.md`, SVG hero banner, and GitHub templates) for the ROM Ingestion Engine.

**Architecture:** Implement a high-impact Cyber-Retro Dark Tech presentation featuring a responsive custom vector SVG banner (`.github/assets/hero-banner.svg`), a comprehensive `README.md` with native GitHub Mermaid architecture diagrams, before/after directory trees, preset comparison matrices, and issue/PR templates.

**Tech Stack:** SVG, GitHub Flavored Markdown, Mermaid.js, GitHub Issue/PR Templates, Shields.io badges.

## Global Constraints
- Target repository: `toilmonkey0-cyber/rom-ingestion-engine`
- Root path: `C:\Users\aaron\Documents\antigravity\optimistic-euclid`
- Aesthetic theme: Cyber-Retro Dark Tech (deep slate `#070d18`, glowing neon cyan `#06b6d4`, electric violet `#8b5cf6`, emerald green `#10b981`)
- No broken links, placeholders, or TBDs
- Strict compliance: ROMs and BIOS files are never bundled, linked, or downloaded
- Existing tests (`npm test`, `cargo test`) must continue to pass

---

### Task 1: Create Custom Vector Hero Banner (`.github/assets/hero-banner.svg`)

**Files:**
- Create: `.github/assets/hero-banner.svg`

**Interfaces:**
- Produces: Vector image referenced by `README.md` at `.github/assets/hero-banner.svg` (1200 × 360 px viewbox).

- [ ] **Step 1: Write test script to validate SVG syntax and element structure**

Create `tools/test_svg.mjs`:
```javascript
import fs from 'node:fs';

const svgPath = '.github/assets/hero-banner.svg';
if (!fs.existsSync(svgPath)) {
  console.error(`Missing SVG file: ${svgPath}`);
  process.exit(1);
}

const content = fs.readFileSync(svgPath, 'utf8');
if (!content.includes('<svg') || !content.includes('</svg>')) {
  console.error('Invalid SVG: missing svg tags');
  process.exit(1);
}
if (!content.includes('ROM INGESTION ENGINE')) {
  console.error('Missing title in SVG');
  process.exit(1);
}
if (!content.includes('Storage Reclaimed')) {
  console.error('Missing key metric pill in SVG');
  process.exit(1);
}
console.log('SVG validation passed!');
```

- [ ] **Step 2: Run test script to verify failure before creation**

Run: `node tools/test_svg.mjs`
Expected: FAIL with "Missing SVG file: .github/assets/hero-banner.svg"

- [ ] **Step 3: Implement `.github/assets/hero-banner.svg`**

Create `.github/assets/hero-banner.svg`:
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 360" width="100%" height="100%">
  <defs>
    <!-- Background Gradient -->
    <linearGradient id="bgGradient" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#070d18"/>
      <stop offset="50%" stop-color="#0b1329"/>
      <stop offset="100%" stop-color="#050811"/>
    </linearGradient>

    <!-- Glowing Cyber Gradients -->
    <linearGradient id="cyanPurple" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#06b6d4"/>
      <stop offset="50%" stop-color="#3b82f6"/>
      <stop offset="100%" stop-color="#8b5cf6"/>
    </linearGradient>

    <linearGradient id="discShine" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#22d3ee" stop-opacity="0.8"/>
      <stop offset="25%" stop-color="#818cf8" stop-opacity="0.3"/>
      <stop offset="50%" stop-color="#c084fc" stop-opacity="0.9"/>
      <stop offset="75%" stop-color="#38bdf8" stop-opacity="0.2"/>
      <stop offset="100%" stop-color="#06b6d4" stop-opacity="0.8"/>
    </linearGradient>

    <linearGradient id="pillBorder" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#0891b2" stop-opacity="0.6"/>
      <stop offset="100%" stop-color="#7c3aed" stop-opacity="0.6"/>
    </linearGradient>

    <!-- Glow Filter -->
    <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="8" result="blur"/>
      <feComposite in="SourceGraphic" in2="blur" operator="over"/>
    </filter>

    <filter id="subtleGlow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="3" result="blur"/>
      <feComposite in="SourceGraphic" in2="blur" operator="over"/>
    </filter>
  </defs>

  <!-- Background Base -->
  <rect width="1200" height="360" rx="16" fill="url(#bgGradient)"/>
  <rect width="1200" height="360" rx="16" fill="none" stroke="#1e293b" stroke-width="2"/>

  <!-- Subtle Cyber Tech Grid Pattern -->
  <g opacity="0.08" stroke="#38bdf8" stroke-width="1">
    <line x1="0" y1="60" x2="1200" y2="60"/>
    <line x1="0" y1="120" x2="1200" y2="120"/>
    <line x1="0" y1="180" x2="1200" y2="180"/>
    <line x1="0" y1="240" x2="1200" y2="240"/>
    <line x1="0" y1="300" x2="1200" y2="300"/>
    <line x1="150" y1="0" x2="150" y2="360"/>
    <line x1="300" y1="0" x2="300" y2="360"/>
    <line x1="450" y1="0" x2="450" y2="360"/>
    <line x1="600" y1="0" x2="600" y2="360"/>
    <line x1="750" y1="0" x2="750" y2="360"/>
    <line x1="900" y1="0" x2="900" y2="360"/>
    <line x1="1050" y1="0" x2="1050" y2="360"/>
  </g>

  <!-- Decorative Accent Glows in Background -->
  <circle cx="150" cy="180" r="140" fill="#06b6d4" opacity="0.12" filter="url(#glow)"/>
  <circle cx="1080" cy="100" r="120" fill="#8b5cf6" opacity="0.10" filter="url(#glow)"/>

  <!-- HERO ICON: Glowing CD/GD-ROM Disc Glyph -->
  <g transform="translate(160, 180)">
    <!-- Outer Rim Glow -->
    <circle cx="0" cy="0" r="105" fill="none" stroke="url(#cyanPurple)" stroke-width="3" filter="url(#subtleGlow)"/>
    
    <!-- Disc Body with Holographic Rainbow Sheen -->
    <circle cx="0" cy="0" r="98" fill="#0f172a" stroke="url(#discShine)" stroke-width="6"/>

    <!-- Concentric Data Tracks -->
    <circle cx="0" cy="0" r="82" fill="none" stroke="#1e293b" stroke-width="1.5" stroke-dasharray="6,4"/>
    <circle cx="0" cy="0" r="68" fill="none" stroke="#334155" stroke-width="1"/>
    <circle cx="0" cy="0" r="54" fill="none" stroke="#0ea5e9" stroke-width="1" stroke-dasharray="14,8" opacity="0.6"/>
    
    <!-- Holographic Light Sweep -->
    <path d="M -85,-35 A 95 95 0 0 1 85,-35 L 0,0 Z" fill="url(#discShine)" opacity="0.15"/>
    <path d="M 85,35 A 95 95 0 0 1 -85,35 L 0,0 Z" fill="url(#discShine)" opacity="0.15"/>

    <!-- Center Clamp Area & Spindle Ring -->
    <circle cx="0" cy="0" r="34" fill="#090d16" stroke="#475569" stroke-width="2"/>
    <circle cx="0" cy="0" r="26" fill="none" stroke="#22d3ee" stroke-width="1.5" opacity="0.8"/>
    <circle cx="0" cy="0" r="14" fill="#020617" stroke="#06b6d4" stroke-width="2.5" filter="url(#subtleGlow)"/>

    <!-- Laser Pick-Up / Read Lens Flare -->
    <circle cx="48" cy="-48" r="4" fill="#22d3ee" filter="url(#subtleGlow)"/>
    <line x1="42" y1="-48" x2="54" y2="-48" stroke="#ffffff" stroke-width="1"/>
    <line x1="48" y1="-54" x2="48" y2="-42" stroke="#ffffff" stroke-width="1"/>
  </g>

  <!-- TYPOGRAPHY SECTION -->
  <g transform="translate(320, 0)">
    <!-- Small Category Kicker -->
    <g transform="translate(0, 78)">
      <rect x="0" y="-18" width="168" height="24" rx="12" fill="#083344" stroke="#0e7490" stroke-width="1"/>
      <text x="84" y="-2" fill="#22d3ee" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif" font-size="11" font-weight="700" letter-spacing="1.5" text-anchor="middle">
        TAURI V2 · RUST CORE
      </text>
    </g>

    <!-- Main Title -->
    <text x="0" y="142" fill="#ffffff" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif" font-size="44" font-weight="900" letter-spacing="-0.5">
      ROM INGESTION ENGINE
    </text>

    <!-- Gradient Accent Subtitle -->
    <text x="0" y="182" fill="url(#cyanPurple)" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif" font-size="19" font-weight="600" letter-spacing="0.2">
      Lossless CHD Compression · Auto-M3U Multi-Disc · 3-Tier Classification
    </text>

    <!-- Description Line -->
    <text x="0" y="214" fill="#94a3b8" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif" font-size="14" font-weight="400">
      Turn chaotic multi-track disc dumps into clean, scrapable, frontend-ready retro libraries.
    </text>

    <!-- KEY STAT PILLS / VALUE BADGES -->
    <g transform="translate(0, 252)">
      <!-- Pill 1: Storage -->
      <g transform="translate(0, 0)">
        <rect x="0" y="0" width="180" height="38" rx="8" fill="#091428" stroke="url(#pillBorder)" stroke-width="1.2"/>
        <text x="14" y="24" fill="#22d3ee" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="14">⚡</text>
        <text x="34" y="24" fill="#f1f5f9" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="12" font-weight="600">~50% Space Reclaimed</text>
      </g>

      <!-- Pill 2: Auto M3U -->
      <g transform="translate(192, 0)">
        <rect x="0" y="0" width="186" height="38" rx="8" fill="#091428" stroke="url(#pillBorder)" stroke-width="1.2"/>
        <text x="14" y="24" fill="#818cf8" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="14">💿</text>
        <text x="34" y="24" fill="#f1f5f9" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="12" font-weight="600">Auto-M3U Playlists</text>
      </g>

      <!-- Pill 3: Needle AI -->
      <g transform="translate(390, 0)">
        <rect x="0" y="0" width="194" height="38" rx="8" fill="#091428" stroke="url(#pillBorder)" stroke-width="1.2"/>
        <text x="14" y="24" fill="#c084fc" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="14">🧠</text>
        <text x="34" y="24" fill="#f1f5f9" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="12" font-weight="600">Needle 3 On-Device AI</text>
      </g>

      <!-- Pill 4: Dry-Run Safety -->
      <g transform="translate(596, 0)">
        <rect x="0" y="0" width="198" height="38" rx="8" fill="#091428" stroke="url(#pillBorder)" stroke-width="1.2"/>
        <text x="14" y="24" fill="#34d399" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="14">🛡️</text>
        <text x="34" y="24" fill="#f1f5f9" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="12" font-weight="600">100% Non-Destructive</text>
      </g>
    </g>
  </g>
</svg>
```

- [ ] **Step 4: Run validation script**

Run: `node tools/test_svg.mjs`
Expected: PASS with "SVG validation passed!"

- [ ] **Step 5: Clean up scratch test script & commit**

```bash
rm tools/test_svg.mjs
git add .github/assets/hero-banner.svg
git commit -m "feat(branding): add responsive cyber-retro SVG hero banner"
```

---

### Task 2: Create GitHub Issue & Pull Request Templates

**Files:**
- Create: `.github/ISSUE_TEMPLATE/bug_report.md`
- Create: `.github/ISSUE_TEMPLATE/feature_request.md`
- Create: `.github/PULL_REQUEST_TEMPLATE.md`

- [ ] **Step 1: Create `.github/ISSUE_TEMPLATE/bug_report.md`**

```markdown
---
name: Bug Report
about: Create a report to help us fix an issue with scanning, classification, or compression
title: "[BUG] "
labels: bug
assignees: ''
---

**Describe the Bug**
A clear and concise description of what went wrong.

**Target Frontend & Preset**
- Preset used: [e.g. OnionOS/GarlicOS, Anbernic Stock OS, ES-DE, Batocera/Knulli, Custom]
- Handheld / Target Device: [e.g. Miyoo Mini Plus, Anbernic RG35XX, Steam Deck, PC]

**Dump Details**
- Game Title:
- Platform: [e.g. PSX, Sega Saturn, Dreamcast, Sega CD, PC Engine CD]
- File format: [e.g. Multi-track .bin/.cue, single .iso, .gdi, orphaned .bin]
- Multi-disc: [Yes / No]

**Environment**
- OS: [e.g. Windows 11, macOS Sequoia (Apple Silicon), Ubuntu 24.04]
- App Version: [e.g. v0.1.0]
- chdman Source: [Auto-installed via 1-Click / System PATH / Custom]

**To Reproduce**
Steps to reproduce the behavior:
1. Go to 'Step 1: Setup & Preset'
2. Select input directory containing '...'
3. Click 'Scan & Plan'
4. See error in Step 2 or during Step 3 compression.

**Expected Behavior**
What you expected to happen.

**Logs or Screenshots**
If applicable, add screenshots or paste terminal/console logs.
```

- [ ] **Step 2: Create `.github/ISSUE_TEMPLATE/feature_request.md`**

```markdown
---
name: Feature Request
about: Suggest an idea, new frontend preset, or disc format support
title: "[FEATURE] "
labels: enhancement
assignees: ''
---

**Is your feature request related to a problem? Please describe.**
A clear and concise description of the friction or missing capability.

**Describe the Solution You'd Like**
A clear and concise description of what you want to happen.

**Target Preset / Hardware Ecosystem**
If requesting a new frontend preset or handheld folder layout, provide:
- Frontend name:
- ROM subfolder convention:
- Multi-disc folder rule (e.g. `.discs/`, subfolder):
- Box art / image naming schema:

**Describe Alternatives You've Considered**
Any alternative solutions or workarounds you've explored.

**Additional Context**
Add any other context, screenshots, or reference docs about the feature.
```

- [ ] **Step 3: Create `.github/PULL_REQUEST_TEMPLATE.md`**

```markdown
## Description
Briefly explain the goal and scope of this pull request.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] New frontend preset or platform mapping
- [ ] Documentation update
- [ ] Performance improvement or refactor

## Invariants & Safety Verification
- [ ] **Non-Destructive Guarantee**: Verified that input directory operations remain read-only.
- [ ] **Atomic Compression**: Verified that `.part` staging and `MComprHD` magic verification are preserved.
- [ ] **No ROMs / BIOS Bundled**: Verified no proprietary or copyrighted binaries/dumps are committed.
- [ ] **Local Inference**: Any classifier changes preserve offline execution and zero-telemetry rules.

## Testing Checklist
- [ ] `npm test` passes (all Vitest frontend tests green)
- [ ] `cargo test` in `src-tauri` passes (all backend suites green)
```

- [ ] **Step 4: Commit GitHub templates**

```bash
git add .github/ISSUE_TEMPLATE/ .github/PULL_REQUEST_TEMPLATE.md
git commit -m "chore(github): add issue and pull request templates"
```

---

### Task 3: Author Flagship `README.md`

**Files:**
- Create: `README.md`

- [ ] **Step 1: Create the comprehensive `README.md`**

Author `README.md` with:
- Top-level hero banner vector link
- Shields/badges bar (Tauri 2, Rust 2021, React 19, CHD v5, Needle 3 AI, Tests, Platforms, MIT, ROMs policy)
- High-impact value proposition paragraph
- Table of Contents
- The Problem & Before/After ASCII directory trees
- Architecture & Pipeline (native Mermaid flowchart + descriptions)
- Feature Pillars (Lossless CHD v5, 3-Tier Classification, Multi-Disc M3U, 1-Click chdman, Libretro box art, Dry-Run Safety)
- Frontend Preset Compatibility Matrix (table)
- The 4-Step User Journey Walkthrough
- Supported Formats & Platforms
- Safety, Privacy & Legal Statement
- Build From Source & Developer Instructions
- Contributing & License

- [ ] **Step 2: Validate markdown formatting and internal anchor links**

Verify that all internal anchors (`#the-problem`, `#architecture--pipeline`, `#frontend-preset-matrix`, etc.) match section headings.

- [ ] **Step 3: Run existing test suites to confirm zero regressions**

Run: `npm test`
Expected: 6 test files passed, 39 tests passed.

- [ ] **Step 4: Commit `README.md`**

```bash
git add README.md
git commit -m "docs: author flagship repository README with architecture and showcase"
```

---

### Task 4: Final Verification & Repository Integrity Check

**Files:**
- Check: `git status`, test suite status

- [ ] **Step 1: Run frontend test suite**
Run: `npm test`
Expected: 39 passing tests.

- [ ] **Step 2: Run backend test compilation check**
Run: `cd src-tauri && cargo check --tests`
Expected: clean compilation, no errors.

- [ ] **Step 3: Review git status and log**
Verify all changes are committed and cleanly structured.
