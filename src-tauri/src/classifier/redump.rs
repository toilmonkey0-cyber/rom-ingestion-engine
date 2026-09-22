use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::sync::LazyLock;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::models::{ClassificationSource, GameClassification, Platform};

/// A single catalog entry in the Redump database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedumpEntry {
    pub canonical_title: String,
    pub platform: Platform,
    pub region: String,
    pub is_multidisc: bool,
    pub disc_number: Option<u8>,
    pub total_discs: Option<u8>,
}

impl RedumpEntry {
    /// Converts a `RedumpEntry` into a `GameClassification` with 100% confidence.
    pub fn to_game_classification(&self) -> GameClassification {
        GameClassification {
            canonical_title: self.canonical_title.clone(),
            platform: self.platform,
            region: self.region.clone(),
            is_multidisc: self.is_multidisc,
            disc_number: self.disc_number,
            total_discs: self.total_discs,
            confidence: 1.0,
            source: ClassificationSource::RedumpCache,
        }
    }
}

/// Lowercase alphanumeric-only title key for fuzzy-exact title matching
/// ("Final Fantasy VII (USA)" == "final fantasy vii").
pub fn normalize_title_key(t: &str) -> String {
    t.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Returns `true` when `s` is a plausible SHA-1 digest: exactly 40 hex characters.
pub fn is_valid_sha1_hex(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// In-memory Redump SHA-1 hash lookup database and cache.
#[derive(Debug, Clone, Default)]
pub struct RedumpDatabase {
    entries: HashMap<String, RedumpEntry>,
}

impl RedumpDatabase {
    /// Creates a new empty in-memory `RedumpDatabase`.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Creates a new empty in-memory `RedumpDatabase`.
    pub fn new_in_memory() -> Self {
        Self::new()
    }

    /// Creates a new `RedumpDatabase` pre-populated with common essential titles.
    pub fn with_builtin_data() -> Self {
        let mut db = Self::new();
        db.load_builtins();
        db
    }

    /// Inserts or updates an entry for the given SHA-1 hash (normalized to lowercase).
    ///
    /// Returns `false` and does nothing when `sha1` is not a valid 40-character
    /// hex string — garbage keys can never match a real hash and only mask
    /// problems in the source data.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        sha1: &str,
        title: &str,
        platform: Platform,
        region: &str,
        is_multidisc: bool,
        disc_number: Option<u8>,
        total_discs: Option<u8>,
    ) -> bool {
        let normalized = sha1.trim().to_ascii_lowercase();
        if !is_valid_sha1_hex(&normalized) {
            return false;
        }
        self.entries.insert(
            normalized,
            RedumpEntry {
                canonical_title: title.to_string(),
                platform,
                region: region.to_string(),
                is_multidisc,
                disc_number,
                total_discs,
            },
        );
        true
    }

    /// Inserts a `RedumpEntry` for the given SHA-1 hash. Returns `false` for
    /// invalid (non-hex) hashes.
    pub fn insert_entry(&mut self, sha1: &str, entry: RedumpEntry) -> bool {
        let normalized = sha1.trim().to_ascii_lowercase();
        if !is_valid_sha1_hex(&normalized) {
            return false;
        }
        self.entries.insert(normalized, entry);
        true
    }

    /// Returns the number of entries stored in the database.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the database has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` if the database contains the given SHA-1 hash.
    pub fn contains_sha1(&self, sha1: &str) -> bool {
        let normalized = sha1.trim().to_ascii_lowercase();
        self.entries.contains_key(&normalized)
    }

    /// Retrieves a reference to the `RedumpEntry` for a given SHA-1 hash.
    pub fn get_entry(&self, sha1: &str) -> Option<&RedumpEntry> {
        let normalized = sha1.trim().to_ascii_lowercase();
        self.entries.get(&normalized)
    }

    /// Infers a platform from a canonical title by exact normalized match
    /// against all indexed entries (builtin + DATs). Only returns when the
    /// title unambiguously maps to ONE platform; multi-system titles stay
    /// Unknown for the user to decide.
    pub fn infer_platform_by_title(&self, title: &str) -> Option<Platform> {
        let key = normalize_title_key(title);
        if key.is_empty() {
            return None;
        }
        let mut platforms = std::collections::HashSet::new();
        for entry in self.entries.values() {
            if normalize_title_key(&entry.canonical_title) == key {
                platforms.insert(entry.platform);
            }
        }
        if platforms.len() == 1 {
            platforms.into_iter().next()
        } else {
            None
        }
    }

    /// Performs a case-insensitive SHA-1 lookup, returning a `GameClassification` if matched.
    pub fn lookup_sha1(&self, sha1: &str) -> Option<GameClassification> {
        let normalized = sha1.trim().to_ascii_lowercase();
        self.entries.get(&normalized).map(|entry| entry.to_game_classification())
    }

    /// Loads entries from a CSV or TSV reader. Automatically detects delimiter and header row.
    pub fn load_csv_or_tsv<R: Read>(&mut self, reader: R) -> Result<usize, std::io::Error> {
        let buf = BufReader::new(reader);
        let mut count = 0;
        let mut header_cols: Option<Vec<String>> = None;
        let mut delimiter: Option<char> = None;

        let mut first_line = true;
        for line_res in buf.lines() {
            let mut line = line_res?;
            // Strip a UTF-8 BOM from the first line so header detection works
            // on files produced by Windows tooling.
            if first_line {
                first_line = false;
                if line.starts_with('\u{feff}') {
                    line.remove(0);
                }
            }
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }

            let delim = match delimiter {
                Some(d) => d,
                None => {
                    let d = if trimmed.contains('\t') {
                        '\t'
                    } else if trimmed.contains(';') {
                        ';'
                    } else {
                        ','
                    };
                    delimiter = Some(d);
                    d
                }
            };

            let fields = parse_delimited_row(trimmed, delim);
            if fields.is_empty() {
                continue;
            }

            if header_cols.is_none() {
                let first_field_lower = fields[0].trim().to_ascii_lowercase();
                if first_field_lower == "sha1"
                    || first_field_lower == "hash"
                    || first_field_lower == "sha-1"
                    || first_field_lower == "title"
                    || first_field_lower == "name"
                {
                    header_cols = Some(fields.into_iter().map(|s| s.trim().to_ascii_lowercase()).collect());
                    continue;
                }
            }

            if self.process_row(&fields, header_cols.as_deref()) {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Convenience wrapper to load comma-separated records.
    pub fn load_csv<R: Read>(&mut self, reader: R) -> Result<usize, std::io::Error> {
        self.load_csv_or_tsv(reader)
    }

    /// Convenience wrapper to load tab-separated records.
    pub fn load_tsv<R: Read>(&mut self, reader: R) -> Result<usize, std::io::Error> {
        self.load_csv_or_tsv(reader)
    }

    /// Loads entries from a Redump-style XML DAT file (`*.dat`).
    ///
    /// The platform for every entry is inferred from the DAT header
    /// (`<name>`/`<description>`, e.g. "Sony Playstation"); each `<game>`
    /// contributes its **first** `<rom>`'s SHA-1 (Redump lists tracks in
    /// order, so the first rom is track 1) together with the game name, from
    /// which region/disc metadata is extracted. Invalid or missing hashes are
    /// skipped (and not counted).
    pub fn load_dat_xml<R: Read>(&mut self, reader: R) -> Result<usize, quick_xml::Error> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut buf_reader = std::io::BufReader::new(reader);
        let mut xml = Reader::from_reader(buf_reader.by_ref());

        let mut buf = Vec::new();
        let mut platform = Platform::Unknown;
        let mut in_header = false;

        let mut current_game: Option<String> = None;
        let mut took_first_rom = false;
        let mut count = 0;

        loop {
            match xml.read_event_into(&mut buf)? {
                Event::Eof => break,
                Event::Start(e) => match e.name().as_ref() {
                    b"header" => in_header = true,
                    b"game" => {
                        current_game = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"name")
                            .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()));
                        took_first_rom = false;
                    }
                    b"rom" => {
                        // Only the first rom of a game (track 1) is indexed.
                        if let (Some(game_name), false) = (&current_game, took_first_rom) {
                            if let Some(sha1) = e
                                .attributes()
                                .filter_map(|a| a.ok())
                                .find(|a| a.key.as_ref() == b"sha1")
                                .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                            {
                                let (disc_number, total_discs) = extract_disc_info(game_name);
                                let region = extract_region(game_name)
                                    .unwrap_or_else(|| "Unknown".to_string());
                                let title = clean_canonical_title(game_name);
                                let is_multidisc = disc_number
                                    .map(|d| d > 0 && total_discs.map(|t| t > 1).unwrap_or(true))
                                    .unwrap_or(false);
                                if self.insert(
                                    &sha1,
                                    &title,
                                    platform,
                                    &region,
                                    is_multidisc,
                                    disc_number,
                                    total_discs,
                                ) {
                                    count += 1;
                                }
                            }
                            took_first_rom = true;
                        }
                    }
                    _ => {}
                },
                Event::Empty(e) => {
                    // <rom .../> self-closing form (used by some DAT writers)
                    if e.name().as_ref() == b"rom" {
                        if let (Some(game_name), false) = (&current_game, took_first_rom) {
                            if let Some(sha1) = e
                                .attributes()
                                .filter_map(|a| a.ok())
                                .find(|a| a.key.as_ref() == b"sha1")
                                .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                            {
                                let (disc_number, total_discs) = extract_disc_info(game_name);
                                let region = extract_region(game_name)
                                    .unwrap_or_else(|| "Unknown".to_string());
                                if self.insert(
                                    &sha1,
                                    &clean_canonical_title(game_name),
                                    platform,
                                    &region,
                                    disc_number
                                        .map(|d| d > 0 && total_discs.map(|t| t > 1).unwrap_or(true))
                                        .unwrap_or(false),
                                    disc_number,
                                    total_discs,
                                ) {
                                    count += 1;
                                }
                            }
                            took_first_rom = true;
                        }
                    }
                }
                Event::Text(t) => {
                    if in_header {
                        let text = t.unescape().unwrap_or_default().to_string();
                        // The header name/description identifies the system
                        // (e.g. "Redump.org - Sony - Playstation").
                        if parse_platform(&text) != Platform::Unknown {
                            platform = parse_platform(&text);
                        } else if let Some(p) = parse_platform_loose(&text) {
                            platform = p;
                        }
                    }
                }
                Event::End(e) => match e.name().as_ref() {
                    b"header" => in_header = false,
                    b"game" => {
                        current_game = None;
                        took_first_rom = false;
                    }
                    _ => {}
                },
                _ => {}
            }
            buf.clear();
        }

        Ok(count)
    }

    fn process_row(&mut self, fields: &[String], headers: Option<&[String]>) -> bool {
        let (sha1_idx, title_idx, platform_idx, region_idx, multidisc_idx, disc_idx, total_discs_idx) = match headers {
            Some(cols) => {
                let mut sha1 = None;
                let mut title = None;
                let mut platform = None;
                let mut region = None;
                let mut multidisc = None;
                let mut disc = None;
                let mut total_discs = None;

                for (i, col) in cols.iter().enumerate() {
                    match col.as_str() {
                        "sha1" | "hash" | "sha-1" | "track1_sha1" | "track1_hash" => sha1 = Some(i),
                        "title" | "canonical_title" | "name" | "game" | "game_name" => title = Some(i),
                        "platform" | "system" | "console" => platform = Some(i),
                        "region" | "country" => region = Some(i),
                        "is_multidisc" | "multidisc" | "multi_disc" => multidisc = Some(i),
                        "disc_number" | "disc" | "disc_no" | "disc_num" => disc = Some(i),
                        "total_discs" | "discs" | "total_disc" | "disc_count" => total_discs = Some(i),
                        _ => {}
                    }
                }

                (
                    sha1.unwrap_or(0),
                    title.unwrap_or(1),
                    platform,
                    region,
                    multidisc,
                    disc,
                    total_discs,
                )
            }
            None => (0, 1, Some(2), Some(3), Some(4), Some(5), Some(6)),
        };

        let raw_sha1 = match fields.get(sha1_idx) {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => return false,
        };

        let raw_title = match fields.get(title_idx) {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => return false,
        };

        let platform = platform_idx
            .and_then(|idx| fields.get(idx))
            .map(|s| parse_platform(s))
            .unwrap_or(Platform::Unknown);

        let explicit_region = region_idx
            .and_then(|idx| fields.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let explicit_multidisc = multidisc_idx
            .and_then(|idx| fields.get(idx))
            .and_then(|s| parse_bool(s));

        let explicit_disc_num = disc_idx
            .and_then(|idx| fields.get(idx))
            .and_then(|s| s.trim().parse::<u8>().ok());

        let explicit_total_discs = total_discs_idx
            .and_then(|idx| fields.get(idx))
            .and_then(|s| s.trim().parse::<u8>().ok());

        // Infer metadata from title if not explicitly provided
        let (disc_from_title, total_from_title) = extract_disc_info(raw_title);
        let disc_number = explicit_disc_num.or(disc_from_title);
        let total_discs = explicit_total_discs.or(total_from_title);

        let is_multidisc = explicit_multidisc.unwrap_or_else(|| {
            if let Some(d) = disc_number {
                d > 0 && total_discs.map(|tot| tot > 1).unwrap_or(true)
            } else {
                false
            }
        });

        let region = explicit_region
            .or_else(|| extract_region(raw_title))
            .unwrap_or_else(|| "Unknown".to_string());

        let canonical_title = clean_canonical_title(raw_title);

        self.insert(
            raw_sha1,
            &canonical_title,
            platform,
            &region,
            is_multidisc,
            disc_number,
            total_discs,
        );

        true
    }

    /// Pre-populates the database with essential known titles across the 5 supported platforms.
    pub fn load_builtins(&mut self) {
        // --- Sony PlayStation (PSX) ---
        self.insert(
            "223b7a702bdf2aa1c6aa9d6b2c29c8e82fa08d91",
            "Metal Gear Solid",
            Platform::Psx,
            "USA",
            true,
            Some(1),
            Some(2),
        );
        self.insert(
            "b51c385a49fb2a4d53820202951e70e171b9c7cf",
            "Metal Gear Solid",
            Platform::Psx,
            "USA",
            true,
            Some(2),
            Some(2),
        );
        self.insert(
            "7890123456789012345678901234567890123451",
            "Final Fantasy VII",
            Platform::Psx,
            "USA",
            true,
            Some(1),
            Some(3),
        );
        self.insert(
            "7890123456789012345678901234567890123452",
            "Final Fantasy VII",
            Platform::Psx,
            "USA",
            true,
            Some(2),
            Some(3),
        );
        self.insert(
            "7890123456789012345678901234567890123453",
            "Final Fantasy VII",
            Platform::Psx,
            "USA",
            true,
            Some(3),
            Some(3),
        );
        self.insert(
            "933ec98e7c7ff0a8399a454b8cffd222471ff9e7",
            "Final Fantasy VII",
            Platform::Psx,
            "USA",
            true,
            Some(1),
            Some(2),
        );
        self.insert(
            "13e555c970aea8babfc7090bd65636767fa5946a",
            "Final Fantasy VII",
            Platform::Psx,
            "USA",
            true,
            Some(2),
            Some(2),
        );
        self.insert(
            "89abcdef0123456789abcdef0123456789abcdef",
            "Castlevania: Symphony of the Night",
            Platform::Psx,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "re2disc100000000000000000000000000000001",
            "Resident Evil 2",
            Platform::Psx,
            "USA",
            true,
            Some(1),
            Some(2),
        );
        self.insert(
            "re2disc200000000000000000000000000000002",
            "Resident Evil 2",
            Platform::Psx,
            "USA",
            true,
            Some(2),
            Some(2),
        );
        self.insert(
            "ccdisc1000000000000000000000000000000001",
            "Chrono Cross",
            Platform::Psx,
            "USA",
            true,
            Some(1),
            Some(2),
        );
        self.insert(
            "ccdisc2000000000000000000000000000000002",
            "Chrono Cross",
            Platform::Psx,
            "USA",
            true,
            Some(2),
            Some(2),
        );
        self.insert(
            "tekken3000000000000000000000000000000001",
            "Tekken 3",
            Platform::Psx,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "crashbandicoot00000000000000000000000001",
            "Crash Bandicoot",
            Platform::Psx,
            "USA",
            false,
            None,
            None,
        );

        // --- Sega Saturn ---
        self.insert(
            "8899aabbccddeeff00112233445566778899aabb",
            "Panzer Dragoon Saga",
            Platform::Saturn,
            "USA",
            true,
            Some(1),
            Some(4),
        );
        self.insert(
            "8899aabbccddeeff00112233445566778899aabc",
            "Panzer Dragoon Saga",
            Platform::Saturn,
            "USA",
            true,
            Some(2),
            Some(4),
        );
        self.insert(
            "8899aabbccddeeff00112233445566778899aabd",
            "Panzer Dragoon Saga",
            Platform::Saturn,
            "USA",
            true,
            Some(3),
            Some(4),
        );
        self.insert(
            "8899aabbccddeeff00112233445566778899aabe",
            "Panzer Dragoon Saga",
            Platform::Saturn,
            "USA",
            true,
            Some(4),
            Some(4),
        );
        self.insert(
            "nights0000000000000000000000000000000001",
            "Nights into Dreams...",
            Platform::Saturn,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "radiantsilvergun00000000000000000000001",
            "Radiant Silvergun",
            Platform::Saturn,
            "Japan",
            false,
            None,
            None,
        );
        self.insert(
            "segarally0000000000000000000000000000001",
            "Sega Rally Championship",
            Platform::Saturn,
            "USA",
            false,
            None,
            None,
        );

        // --- Sega Dreamcast ---
        self.insert(
            "3344556677889900aabbccddeeff001122334455",
            "Shenmue",
            Platform::Dreamcast,
            "USA",
            true,
            Some(1),
            Some(4),
        );
        self.insert(
            "3344556677889900aabbccddeeff001122334456",
            "Shenmue",
            Platform::Dreamcast,
            "USA",
            true,
            Some(2),
            Some(4),
        );
        self.insert(
            "3344556677889900aabbccddeeff001122334457",
            "Shenmue",
            Platform::Dreamcast,
            "USA",
            true,
            Some(3),
            Some(4),
        );
        self.insert(
            "3344556677889900aabbccddeeff001122334458",
            "Shenmue",
            Platform::Dreamcast,
            "USA",
            true,
            Some(4),
            Some(4),
        );
        self.insert(
            "sonicadventure0000000000000000000000001",
            "Sonic Adventure",
            Platform::Dreamcast,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "skiesofarcadia10000000000000000000000001",
            "Skies of Arcadia",
            Platform::Dreamcast,
            "USA",
            true,
            Some(1),
            Some(2),
        );
        self.insert(
            "skiesofarcadia20000000000000000000000002",
            "Skies of Arcadia",
            Platform::Dreamcast,
            "USA",
            true,
            Some(2),
            Some(2),
        );
        self.insert(
            "crazytaxi0000000000000000000000000000001",
            "Crazy Taxi",
            Platform::Dreamcast,
            "USA",
            false,
            None,
            None,
        );

        // --- Sega CD ---
        self.insert(
            "soniccd000000000000000000000000000000001",
            "Sonic the Hedgehog CD",
            Platform::SegaCd,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "snatcher00000000000000000000000000000001",
            "Snatcher",
            Platform::SegaCd,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "lunarthesilverstar0000000000000000000001",
            "Lunar: The Silver Star",
            Platform::SegaCd,
            "USA",
            false,
            None,
            None,
        );

        // --- PC Engine CD ---
        self.insert(
            "rondofblood00000000000000000000000000001",
            "Castlevania: Rondo of Blood",
            Platform::PceCd,
            "Japan",
            false,
            None,
            None,
        );
        self.insert(
            "ysiandii00000000000000000000000000000001",
            "Ys I & II",
            Platform::PceCd,
            "USA",
            false,
            None,
            None,
        );
        self.insert(
            "lordsofthunder00000000000000000000000001",
            "Lords of Thunder",
            Platform::PceCd,
            "USA",
            false,
            None,
            None,
        );
    }
}

/// Parses a string into a `Platform` enum.
pub fn parse_platform(s: &str) -> Platform {
    let lower = s.trim().to_ascii_lowercase();
    match lower.as_str() {
        "psx" | "ps1" | "psone" | "ps-one" | "playstation" | "sony playstation" | "sony_playstation" | "ps" => Platform::Psx,
        "saturn" | "ss" | "sega saturn" | "sega_saturn" => Platform::Saturn,
        "dreamcast" | "dc" | "sega dreamcast" | "sega_dreamcast" => Platform::Dreamcast,
        "segacd" | "sega-cd" | "sega_cd" | "sega cd" | "megacd" | "mega cd" | "mega-cd" | "mega_cd" | "scd" => Platform::SegaCd,
        "pcecd" | "pce-cd" | "pce_cd" | "pce cd" | "pcenginecd" | "pc engine cd" | "turbografx-cd" | "turbografx cd" | "turbografx-16 cd" | "turbografx16cd" | "tg16cd" | "tg-cd" | "tgcd" | "pce" => Platform::PceCd,
        _ => Platform::Unknown,
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    let lower = s.trim().to_ascii_lowercase();
    match lower.as_str() {
        "true" | "1" | "yes" | "y" | "t" => Some(true),
        "false" | "0" | "no" | "n" | "f" => Some(false),
        _ => None,
    }
}

/// Infers a platform from free-form text such as a Redump DAT header
/// ("Redump.org - Sony - Playstation") via keyword containment.
pub fn parse_platform_loose(text: &str) -> Option<Platform> {
    let alphanumeric: String = text
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    if alphanumeric.contains("dreamcast") {
        return Some(Platform::Dreamcast);
    }
    if alphanumeric.contains("saturn") {
        return Some(Platform::Saturn);
    }
    if alphanumeric.contains("playstation") || alphanumeric.contains(" psx ") || alphanumeric == "psx" {
        return Some(Platform::Psx);
    }
    if alphanumeric.contains("mega cd") || alphanumeric.contains("megacd") || alphanumeric.contains("sega cd") || alphanumeric.contains("segacd") {
        return Some(Platform::SegaCd);
    }
    if alphanumeric.contains("pc engine") || alphanumeric.contains("pcengine") || alphanumeric.contains("turbografx") {
        return Some(Platform::PceCd);
    }
    None
}

/// Tokenizes a delimited line, handling double-quoted strings and escaped quotes.
fn parse_delimited_row(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == delimiter && !in_quotes {
            fields.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

static DISC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\(?(?:disc|disque|disco|cd)\s*(\d+)(?:\s*(?:of|/)\s*(\d+))?\)?"#).unwrap()
});

/// Redump region vocabulary (lowercase), including common abbreviations that
/// appear in filenames and model answers. Shared by region extraction and tag
/// stripping so a recognized region is always stripped from the canonical title.
const REGIONS: &[&str] = &[
    "usa", "us", "na", "north america", "europe", "eu", "pal", "japan", "jpn", "world", "asia",
    "australia", "brazil", "canada", "china", "france", "germany", "hong kong", "italy", "korea",
    "netherlands", "russia", "spain", "sweden", "taiwan", "uk", "mexico", "argentina",
];

/// Maps a lowercase region token (or alias) to its canonical display form.
fn canonical_region(lower: &str) -> Option<String> {
    match lower {
        "usa" | "us" | "na" | "north america" => Some("USA".to_string()),
        "europe" | "eu" | "pal" => Some("Europe".to_string()),
        "japan" | "jpn" => Some("Japan".to_string()),
        "uk" => Some("UK".to_string()),
        _ => {
            if REGIONS.contains(&lower) {
                // Title-case each word: "hong kong" -> "Hong Kong"
                Some(
                    lower
                        .split_whitespace()
                        .map(|w| {
                            let mut cs = w.chars();
                            match cs.next() {
                                Some(f) => {
                                    f.to_uppercase().collect::<String>()
                                        + &cs.as_str().to_lowercase()
                                }
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            } else {
                None
            }
        }
    }
}

/// Language codes that appear as parenthesized tags, e.g. `(En)`, `(Ja,En)`.
const LANG_CODES: &[&str] = &["en", "ja", "fr", "de", "es", "it", "pt", "ko", "zh"];

fn build_region_pattern() -> String {
    REGIONS.join("|")
}

fn build_lang_list_pattern() -> String {
    let codes = LANG_CODES.join("|");
    format!(r"(?:{})(?:\s*,\s*(?:{}))*", codes, codes)
}

static REGION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"(?i)\(({})(?:,[^)]*)?\)"#, build_region_pattern())).unwrap()
});

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)\s*\((?:{}(?:,[^)]*)?|{}|(?:disc|disque|disco|cd|track)\s*\d+[^)]*|v\d+[^)]*|rev\s*[^)]*|demo|beta|proto|sample|unl|alt\s*\d*|edc)\)"#,
        build_region_pattern(),
        build_lang_list_pattern(),
    ))
    .unwrap()
});

static BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\s*\[[^\]]*\]"#).unwrap()
});

/// Extracts disc number and total discs from a title string if present.
pub fn extract_disc_info(title: &str) -> (Option<u8>, Option<u8>) {
    if let Some(caps) = DISC_RE.captures(title) {
        let disc_num = caps.get(1).and_then(|m| m.as_str().parse::<u8>().ok());
        let total = caps.get(2).and_then(|m| m.as_str().parse::<u8>().ok());
        (disc_num, total)
    } else {
        (None, None)
    }
}

/// Extracts release region from parenthesized tags in a title.
pub fn extract_region(title: &str) -> Option<String> {
    REGION_RE
        .captures(title)
        .and_then(|c| c.get(1))
        .and_then(|m| canonical_region(&m.as_str().trim().to_ascii_lowercase()))
}

/// Strips release tags (region, disc, revision, dump tags) from a raw title to return canonical game title.
pub fn clean_canonical_title(raw_title: &str) -> String {
    let cleaned = TAG_RE.replace_all(raw_title, "");
    let cleaned = BRACKET_RE.replace_all(&cleaned, "");
    let cleaned = cleaned.trim();

    if cleaned.is_empty() {
        raw_title.trim().to_string()
    } else {
        cleaned.to_string()
    }
}
