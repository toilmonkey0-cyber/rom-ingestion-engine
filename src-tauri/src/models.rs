use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Set when this descriptor could not be paired safely. The rest of the folder still plans.
    #[serde(default)]
    pub scan_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontendPreset {
    EsDe,
    OnionOs,
    AnbernicStock,
    Batocera,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Forward-slash playlist entry for this disc, relative to the M3U location.
    #[serde(default)]
    pub relative_m3u_entry: Option<String>,
    pub status: TaskStatus,
    /// Tracks resolved at scan time. Empty means execute may re-read the cue.
    #[serde(default)]
    pub binary_tracks: Vec<PathBuf>,
    /// `createcd` or `createdvd`. Empty means derive it from the descriptor extension.
    #[serde(default)]
    pub chdman_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    BoxArt,
    Screenshots,
    TitleScreens,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Jev or scan advisory the dry-run must show. Empty when classification succeeded.
    #[serde(default)]
    pub status_note: Option<String>,
    /// `keeper` or `alternate`. Empty means keeper.
    #[serde(default)]
    pub role: String,
    pub artwork_url: Option<String>,
    pub target_media_paths: Vec<PathBuf>,
}

/// A disc discovered during scanning that was excluded from the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedSource {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionPlan {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub preset: FrontendPreset,
    pub games: Vec<PlannedGame>,
    /// Discs found during scanning but excluded (missing tracks, references
    /// escaping the scan root, unreadable sheets). Surfaced for the dry-run UI.
    #[serde(default)]
    pub skipped_sources: Vec<SkippedSource>,
    pub total_source_bytes: u64,
    pub estimated_output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobProgressEvent {
    pub game_id: String,
    pub disc_number: u8,
    pub progress: f32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameStatusEvent {
    pub game_id: String,
    pub status: TaskStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashOutcome {
    pub count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub total_games: usize,
    pub successful_games: usize,
    pub failed_games: usize,
    pub total_discs: usize,
    pub processed_discs: usize,
    pub source_files_to_trash: Vec<String>,
    pub total_source_bytes: u64,
    pub total_output_bytes: u64,
    #[serde(default)]
    pub failed_game_ids: Vec<String>,
    #[serde(default)]
    pub partial_game_ids: Vec<String>,
}

/// Status of one game's artwork lookup during the Finish Line phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkStatus {
    Downloading,
    Done,
    Skipped,
    Failed,
}

/// Emitted per game while box art is being fetched and written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtworkProgressEvent {
    pub game_id: String,
    pub title: String,
    pub status: ArtworkStatus,
    pub completed: usize,
    pub total: usize,
}

/// Outcome of the Finish Line phase (artwork + playlist metadata).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishLibrarySummary {
    pub gamelists_written: usize,
    pub artwork_downloaded: usize,
    pub artwork_skipped: usize,
    pub artwork_failed: usize,
    /// Paths of artwork present on disk after the phase (downloaded or
    /// previously existing), for UI preview.
    #[serde(default)]
    pub artwork_paths: Vec<String>,
}

/// Emitted per file while a preset migration is being executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationProgressEvent {
    pub message: String,
    pub completed: usize,
    pub total: usize,
}
