# GitHub Repository Page Design Specification — ROM Ingestion Engine

**Date:** 2026-09-27  
**Status:** Approved  
**Aesthetic Theme:** Cyber-Retro Dark Tech  
**Target Repository:** `toilmonkey0-cyber/rom-ingestion-engine`

---

## 1. Executive Summary & Objective

The **ROM Ingestion Engine** is a high-performance desktop application built with Tauri v2, Rust, React 19, and Tailwind CSS. It automates the discovery, multi-tier classification, lossless CHD v5 compression, multi-disc `.m3u` playlist generation, and Libretro box art ingestion for retro handheld gaming libraries (OnionOS, Anbernic Stock OS, ES-DE, Batocera/Knulli).

The objective of this specification is to establish a world-class, professional, and eye-catching GitHub repository presentation that immediately communicates the project's technical moat, ease of use, safety guarantees, and ecosystem value to retro handheld owners, emulation enthusiasts, and open-source contributors.

---

## 2. Visual Identity & Assets

### 2.1 Color Palette & Aesthetic
- **Canvas Base:** Deep slate / void dark (`#070d18`, `#0b0f19`)
- **Primary Cyber Accent:** Neon Cyan (`#06b6d4`, `#22d3ee`)
- **Secondary Accent:** Electric Violet (`#8b5cf6`, `#a855f7`)
- **Success / Verification:** Emerald Green (`#10b981`, `#00d26a`)
- **Warning / Review:** Amber Gold (`#f59e0b`)

### 2.2 Hero Banner Asset (`.github/assets/hero-banner.svg`)
- **Format:** 1200 × 360 SVG, 100% scalable vector, responsive on both GitHub dark and light themes.
- **Visual Elements:**
  - Optical disc glyph with concentric track geometry, cyan/violet laser gradient ring, and active data sectors.
  - Bold, modern typography: `ROM INGESTION ENGINE`.
  - Subtitle: `Lossless CHD Compression · Auto-M3U Multi-Disc · 3-Tier Intelligent Classification`.
  - Integrated metric pills:
    - `⚡ 40–60% Storage Reclaimed`
    - `💿 1-Game 1-Entry M3U Playlists`
    - `🧠 Needle 3 On-Device AI`
    - `🛡️ 100% Non-Destructive Dry-Run`

### 2.3 Shields & Badges Specification
A clean, centralized badge bar displaying:
- `Tauri v2`: `https://img.shields.io/badge/Tauri-v2-24C8DB?style=flat-square&logo=tauri&logoColor=white`
- `Rust 2021`: `https://img.shields.io/badge/Rust-2021-DEA584?style=flat-square&logo=rust&logoColor=white`
- `React 19`: `https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black`
- `Compression`: `https://img.shields.io/badge/Format-Lossless%20CHD%20v5-00D26A?style=flat-square`
- `AI Tier`: `https://img.shields.io/badge/AI-Needle%203%20Local-8B5CF6?style=flat-square`
- `Test Coverage`: `https://img.shields.io/badge/Tests-58%20Passed-brightgreen?style=flat-square`
- `Platforms`: `https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-0078D6?style=flat-square`
- `License`: `https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square`
- `Legal`: `https://img.shields.io/badge/ROMs%20%26%20BIOS-Never%20Bundled-2F855A?style=flat-square`

---

## 3. README.md Information Architecture

The repository `README.md` is structured into 10 structured sections:

1. **Header & Hero**:
   - Hero banner vector graphic.
   - Shields & status badge bar.
   - High-impact lead paragraph and value proposition.
   - Table of contents.
2. **The Problem & Before/After Transformation**:
   - Problem summary: multi-track `.bin/.cue` fragmentation, duplicate multi-disc entries, missing `.cue` sheets, and uncompressed storage waste.
   - Side-by-side / sequential ASCII directory trees illustrating the raw dump jungle vs. pristine ingested structure.
3. **Architecture & 4-Stage Pipeline**:
   - Native Mermaid flowchart diagram rendering Stage 1 (Discovery & Synthetic CUEs), Stage 2 (Triple-Tier Classification), Stage 3 (Interactive Dry-Run Review), and Stage 4 (Concurrent Compression & Scrape).
   - Core architectural invariants: non-destructive read-only scanning, crash-safe `.part` staging, magic byte verification, and bounded concurrency.
4. **Key Features & Technical Moat**:
   - Lossless CHD v5 compression (40-60% reclaimed space, CD-DA fidelity).
   - Triple-Tier Classification (Redump Hash Cache → Needle 3 Local AI → TypeSafe Jev fallback).
   - Zero-Duplicate Multi-Disc & M3U Orchestration.
   - 1-Click chdman Auto-Downloader (cross-platform SHA-256 verified).
   - Automated Libretro Box Art & Media Ingestion (zero-API-key thumbnails mapped to frontend conventions).
   - Dry-Run Safety & Recycle Bin (`trash` crate) ledger.
5. **Frontend Preset Compatibility Matrix**:
   - Comprehensive table detailing folder paths and media locations for OnionOS/GarlicOS, Anbernic Stock OS, ES-DE, Batocera/Knulli, and Custom layouts.
6. **The 4-Step User Journey**:
   - Visual breakdown of Wizard Steps: Setup & Preset → Dry-Run Review → High-Throughput Compression → Summary & Ledger.
7. **Supported Disc Formats & Platforms**:
   - Table covering PSX, Sega Saturn, Dreamcast, Sega CD, PC Engine CD with supported descriptor types (`.cue`, `.iso`, `.gdi`, `.toc`, orphaned `.bin`).
8. **Safety, Privacy & Legal Notice**:
   - Non-destructive guarantee.
   - ROMs & BIOS compliance (never bundled, hosted, or downloaded).
   - Local-first privacy: Needle 3 local AI runs 100% on-device with zero telemetry (`NEEDLE_TELEMETRY=0`, `DO_NOT_TRACK=1`).
9. **Build From Source & Developer Guide**:
   - System requirements (Node 20+, Rust stable, C++ build tools).
   - Quickstart commands (`npm install`, `npm run tauri dev`).
   - Running test suites (`npm test` for Vitest, `cargo test` for Rust backend).
10. **Contributing & License**:
    - Contributing guidelines, bug reporting, and MIT license terms.

---

## 4. GitHub Repository Infrastructure

### 4.1 Issue Templates
- `.github/ISSUE_TEMPLATE/bug_report.md`:
  - Structured fields for Operating System, Frontend Preset, Input Dump Format (`.cue/.bin`, `.gdi`, `.iso`, orphaned `.bin`), Game Title / Region, chdman status, and terminal/debug logs.
- `.github/ISSUE_TEMPLATE/feature_request.md`:
  - Structured fields for Proposed Feature, Target Frontend / Platform, Problem Solved, and Alternative Workarounds.

### 4.2 Pull Request Template
- `.github/PULL_REQUEST_TEMPLATE.md`:
  - Summary of changes.
  - Linked issue.
  - Verification checklist (passes `cargo test`, passes `npm test`, non-destructive guarantee verified, no ROMs/BIOS bundled).

---

## 5. Verification & Self-Review Checklist

- [x] **Placeholder scan**: All URLs, paths, and commands are concrete and point to `toilmonkey0-cyber/rom-ingestion-engine`.
- [x] **Consistency**: Aligns with actual Rust and TypeScript implementations (`src-tauri` and `src`).
- [x] **Scope check**: Delivers complete repository page design, SVG hero banner, README.md, and GitHub workflow templates.
- [x] **Ambiguity check**: Preset paths and classification tiers precisely reflect the codebase.
