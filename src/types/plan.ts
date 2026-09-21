export type Platform = 'psx' | 'saturn' | 'dreamcast' | 'segacd' | 'pcecd' | 'unknown';

export type FrontendPreset = 'esde' | 'onionos' | 'anbernicstock' | 'batocera' | 'custom';

export type TaskStatus = 'pending' | 'compressing' | 'verified' | 'failed' | 'skipped';

export type ClassificationSource = 'redumpcache' | 'jevai' | 'fallback';

export interface PlannedDisc {
  disc_number: number;
  source_descriptor: string;
  target_chd_path: string;
  status: TaskStatus;
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
}

export interface IngestionPlan {
  input_dir: string;
  output_dir: string;
  preset: FrontendPreset;
  games: PlannedGame[];
  total_source_bytes: number;
  estimated_output_bytes: number;
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
