export type Platform = 'psx' | 'saturn' | 'dreamcast' | 'segacd' | 'pcecd' | 'unknown';

export type FrontendPreset = 'esde' | 'onionos' | 'anbernicstock' | 'batocera' | 'custom';

export type TaskStatus = 'pending' | 'compressing' | 'verified' | 'failed' | 'skipped';

export type ClassificationSource = 'redumpcache' | 'jevai' | 'needleai' | 'fallback';

export interface PlannedDisc {
  disc_number: number;
  source_descriptor: string;
  target_chd_path: string;
  relative_m3u_entry?: string | null;
  status: TaskStatus;
}

/** Per-platform folder overrides for the `custom` frontend preset. */
export interface CustomPresetConfig {
  psx: string;
  saturn: string;
  dreamcast: string;
  sega_cd: string;
  pce_cd: string;
  unknown: string;
  multidisc_subfolder: string;
}

export interface SkippedSource {
  path: string;
  reason: string;
}

export interface IngestionPlan {
  input_dir: string;
  output_dir: string;
  preset: FrontendPreset;
  games: PlannedGame[];
  skipped_sources: SkippedSource[];
  total_source_bytes: number;
  estimated_output_bytes: number;
}

export interface PlannedGame {
  id: string;
  canonical_title: string;
  platform: Platform;
  region: string;
  is_multidisc: boolean;
  discs: PlannedDisc[];
  target_m3u_path: string | null;
  confidence: number;
  source: ClassificationSource;
  enabled: boolean;
  needs_review: boolean;
  status_note?: string | null;
  role?: string;
  artwork_url?: string | null;
  target_media_paths?: string[];
}

export interface MediaOptions {
  download_boxart: boolean;
  download_screenshots: boolean;
  download_titles: boolean;
}

export interface JobProgressEvent {
  game_id: string;
  disc_number: number;
  progress: number;
  message: string;
}

export interface GameStatusEvent {
  game_id: string;
  status: TaskStatus;
  error?: string | null;
}

export interface ExecutionSummary {
  total_games: number;
  successful_games: number;
  failed_games: number;
  total_discs: number;
  processed_discs: number;
  source_files_to_trash: string[];
  total_source_bytes: number;
  total_output_bytes: number;
  failed_game_ids?: string[];
  partial_game_ids?: string[];
}

export interface TrashOutcome {
  count: number;
  bytes: number;
}

export type ChdmanSource = 'system_path' | 'managed_directory' | 'custom_path' | 'missing';

export interface ChdmanStatus {
  ready: boolean;
  source: ChdmanSource;
  path: string | null;
  version: string | null;
}

export interface DownloadProgressEvent {
  downloaded_bytes: number;
  total_bytes: number;
  percentage: number;
}

export type ArtworkStatus = 'downloading' | 'done' | 'skipped' | 'failed';

export interface ArtworkProgressEvent {
  game_id: string;
  title: string;
  status: ArtworkStatus;
  completed: number;
  total: number;
}

export interface FinishLibrarySummary {
  gamelists_written: number;
  artwork_downloaded: number;
  artwork_skipped: number;
  artwork_failed: number;
  artwork_paths?: string[];
}

export type MigrationItemKind = 'chd' | 'playlist' | 'artwork' | 'stale_gamelist';

export interface MigrationItem {
  kind: MigrationItemKind;
  source: string;
  target: string;
}

export interface PlaylistRewrite {
  target: string;
  entries: string[];
}

export interface BrokenPlaylist {
  playlist: string;
  missing_entries: string[];
}

export interface MigrationPlan {
  root: string;
  source_preset: FrontendPreset;
  target_preset: FrontendPreset;
  games: number;
  items: MigrationItem[];
  playlist_rewrites: PlaylistRewrite[];
  broken_playlists?: BrokenPlaylist[];
}

export interface MigrationSummary {
  files_moved: number;
  playlists_rewritten: number;
  gamelists_written: number;
  skipped_existing: string[];
}

export interface WatchStatusEvent {
  stage: 'watching' | 'ingesting' | 'done' | 'error' | 'stopped';
  message: string;
}

export interface MigrationProgressEvent {
  message: string;
  completed: number;
  total: number;
}
