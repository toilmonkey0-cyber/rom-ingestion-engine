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
    pub status: TaskStatus,
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
    pub artwork_url: Option<String>,
    pub target_media_paths: Vec<PathBuf>,
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
pub struct ExecutionSummary {
    pub total_games: usize,
    pub successful_games: usize,
    pub failed_games: usize,
    pub total_discs: usize,
    pub processed_discs: usize,
    pub source_files_to_trash: Vec<String>,
    pub total_source_bytes: u64,
    pub total_output_bytes: u64,
}
