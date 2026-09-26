import {
  IngestionPlan,
  ExecutionSummary,
  FrontendPreset,
  JobProgressEvent,
  GameStatusEvent,
  ChdmanStatus,
  DownloadProgressEvent,
  MediaOptions,
  PlannedGame,
  TrashOutcome,
  CustomPresetConfig,
  FinishLibrarySummary,
  ArtworkProgressEvent,
  MigrationPlan,
  MigrationSummary,
  MigrationProgressEvent,
  WatchStatusEvent,
  Platform,
} from '../types/plan';

export const DEFAULT_CUSTOM_PRESET: CustomPresetConfig = {
  psx: 'roms/psx',
  saturn: 'roms/saturn',
  dreamcast: 'roms/dreamcast',
  sega_cd: 'roms/segacd',
  pce_cd: 'roms/pcenginecd',
  unknown: 'roms/unknown',
  multidisc_subfolder: '.discs',
};

export const isTauri = (): boolean => {
  return (
    typeof window !== 'undefined' &&
    Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
};

export async function scanAndPlanApi(
  inputDir: string,
  outputDir: string,
  preset: FrontendPreset,
  apiKey?: string,
  mediaOptions?: MediaOptions,
  datPath?: string,
  regionPriority?: string[],
  customConfig?: CustomPresetConfig | null,
  redumpDatPaths?: string[] | null
): Promise<IngestionPlan> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<IngestionPlan>('scan_and_plan', {
      inputDir,
      outputDir,
      preset,
      apiKey: apiKey?.trim() ? apiKey.trim() : null,
      mediaOptions: mediaOptions ?? null,
      datPath: datPath?.trim() ? datPath.trim() : null,
      regionPriority: regionPriority && regionPriority.length > 0 ? regionPriority : null,
      jevBaseUrl: null,
      needleBaseUrl: null,
      customConfig: preset === 'custom' ? customConfig ?? DEFAULT_CUSTOM_PRESET : null,
      redumpDatPaths:
        redumpDatPaths && redumpDatPaths.length > 0
          ? redumpDatPaths.map((p) => p.trim()).filter(Boolean)
          : null,
    });
  }

  // Realistic mock plan for browser dev / testing
  await new Promise((r) => setTimeout(r, 600));
  return {
    input_dir: inputDir || 'D:/Roms/Incoming',
    output_dir: outputDir || 'D:/Roms/Organized',
    preset,
    total_source_bytes: 5368709120, // 5.0 GB
    estimated_output_bytes: 2952790016, // 2.75 GB (~45% savings)
    skipped_sources: [
      {
        path: 'D:/Roms/Incoming/Broken Game (USA).cue',
        reason: 'Referenced track not found: Broken Game (USA).bin',
      },
    ],
    games: [
      {
        id: 'game-mock-1',
        canonical_title: 'Final Fantasy VII',
        platform: 'psx',
        region: 'USA',
        is_multidisc: true,
        discs: [
          { disc_number: 1, source_descriptor: 'FF7_Disc1.cue', target_chd_path: '.discs/Final Fantasy VII (Disc 1).chd', status: 'pending' },
          { disc_number: 2, source_descriptor: 'FF7_Disc2.cue', target_chd_path: '.discs/Final Fantasy VII (Disc 2).chd', status: 'pending' },
          { disc_number: 3, source_descriptor: 'FF7_Disc3.cue', target_chd_path: '.discs/Final Fantasy VII (Disc 3).chd', status: 'pending' },
        ],
        target_m3u_path: 'Final Fantasy VII.m3u',
        confidence: 0.98,
        source: 'redumpcache',
        enabled: true,
        needs_review: false,
        artwork_url: 'https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final Fantasy VII (USA).png',
        target_media_paths: ['ROMS/PS/Imgs/Final Fantasy VII (USA).png'],
      },
      {
        id: 'game-mock-2',
        canonical_title: 'Castlevania - Symphony of the Night',
        platform: 'psx',
        region: 'USA',
        is_multidisc: false,
        discs: [
          { disc_number: 1, source_descriptor: 'Castlevania.cue', target_chd_path: 'Castlevania - Symphony of the Night.chd', status: 'pending' },
        ],
        target_m3u_path: null,
        confidence: 0.95,
        source: 'redumpcache',
        enabled: true,
        needs_review: false,
        artwork_url: 'https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania - Symphony of the Night (USA).png',
        target_media_paths: ['ROMS/PS/Imgs/Castlevania - Symphony of the Night (USA).png'],
      },
      {
        id: 'game-mock-3',
        canonical_title: 'Panzer Dragoon Saga',
        platform: 'saturn',
        region: 'USA',
        is_multidisc: true,
        discs: [
          { disc_number: 1, source_descriptor: 'PDS_Disc1.cue', target_chd_path: '.discs/Panzer Dragoon Saga (Disc 1).chd', status: 'pending' },
          { disc_number: 2, source_descriptor: 'PDS_Disc2.cue', target_chd_path: '.discs/Panzer Dragoon Saga (Disc 2).chd', status: 'pending' },
          { disc_number: 3, source_descriptor: 'PDS_Disc3.cue', target_chd_path: '.discs/Panzer Dragoon Saga (Disc 3).chd', status: 'pending' },
          { disc_number: 4, source_descriptor: 'PDS_Disc4.cue', target_chd_path: '.discs/Panzer Dragoon Saga (Disc 4).chd', status: 'pending' },
        ],
        target_m3u_path: 'Panzer Dragoon Saga.m3u',
        confidence: 0.92,
        source: 'jevai',
        enabled: true,
        needs_review: false,
      },
      {
        id: 'game-mock-4',
        canonical_title: 'Sonic Adventure 2 Unofficial Undub',
        platform: 'dreamcast',
        region: 'UNKNOWN',
        is_multidisc: false,
        discs: [
          { disc_number: 1, source_descriptor: 'SA2_Undub.gdi', target_chd_path: 'Sonic Adventure 2 Unofficial Undub.chd', status: 'pending' },
        ],
        target_m3u_path: null,
        confidence: 0.68,
        source: 'fallback',
        enabled: true,
        needs_review: true,
      },
    ],
  };
}

export async function executePlanApi(
  plan: IngestionPlan,
  onProgress?: (event: JobProgressEvent) => void,
  onStatus?: (event: GameStatusEvent) => void
): Promise<ExecutionSummary> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    const { listen } = await import('@tauri-apps/api/event');

    const unlistenProgress = await listen<JobProgressEvent>('job-progress', (e) => {
      if (onProgress) onProgress(e.payload);
    });
    const unlistenStatus = await listen<GameStatusEvent>('game-status', (e) => {
      if (onStatus) onStatus(e.payload);
    });

    try {
      return await invoke<ExecutionSummary>('execute_plan', { plan });
    } finally {
      unlistenProgress();
      unlistenStatus();
    }
  }

  // Browser simulation
  const enabledGames = plan.games.filter((g) => g.enabled);
  let processedDiscs = 0;
  const totalDiscs = enabledGames.reduce((acc, g) => acc + g.discs.length, 0);
  const trashFiles: string[] = [];

  for (const game of enabledGames) {
    if (onStatus) {
      onStatus({ game_id: game.id, status: 'compressing' });
    }

    for (const disc of game.discs) {
      trashFiles.push(disc.source_descriptor);
      trashFiles.push(disc.source_descriptor.replace(/\.(cue|gdi)$/i, '.bin'));
      for (let p = 20; p <= 100; p += 40) {
        await new Promise((r) => setTimeout(r, 120));
        if (onProgress) {
          onProgress({
            game_id: game.id,
            disc_number: disc.disc_number,
            progress: p,
            message: `Compressing disc ${disc.disc_number} (${p}%)`,
          });
        }
      }
      processedDiscs++;
    }

    if (onStatus) {
      onStatus({ game_id: game.id, status: 'verified' });
    }
  }

  return {
    total_games: enabledGames.length,
    successful_games: enabledGames.length,
    failed_games: 0,
    total_discs: totalDiscs,
    processed_discs: processedDiscs,
    source_files_to_trash: trashFiles,
    total_source_bytes: plan.total_source_bytes,
    total_output_bytes: plan.estimated_output_bytes,
  };
}

export async function trashSourceFilesApi(
  sourceFiles: string[],
  baseDir?: string | null,
  allowPermanent?: boolean
): Promise<TrashOutcome> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<TrashOutcome>('trash_source_files', {
      sourceFiles,
      baseDir: baseDir?.trim() ? baseDir.trim() : null,
      allowPermanent: allowPermanent === true,
    });
  }

  await new Promise((r) => setTimeout(r, 400));
  return { count: sourceFiles.length, bytes: sourceFiles.length };
}

export async function applyDatReleaseApi(
  game: PlannedGame,
  title: string,
  region: string,
  outputDir: string,
  preset: FrontendPreset
): Promise<PlannedGame> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<PlannedGame>('apply_dat_release', {
      game,
      title,
      region,
      platform: game.platform,
      discNumber: game.discs[0]?.disc_number ?? 1,
      outputDir,
      preset,
    });
  }
  return {
    ...game,
    canonical_title: title,
    region,
    needs_review: false,
    enabled: true,
    status_note: null,
    role: 'keeper',
    discs: game.discs.map((disc) => ({
      ...disc,
      target_chd_path: `${outputDir}/${preset}/${title} (${region}).chd`,
    })),
  };
}

function previewStem(title: string, region: string): string {
  const raw = region && !title.endsWith(`(${region})`) ? `${title} (${region})` : title;
  return raw.replace(/[&*/:\\<>?|"]/g, '_');
}

export async function renamePlannedGameApi(
  game: PlannedGame,
  title: string,
  outputDir: string,
  preset: FrontendPreset,
  mediaOptions?: MediaOptions
): Promise<PlannedGame> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<PlannedGame>('rename_planned_game', {
      game,
      title,
      outputDir,
      preset,
      mediaOptions: mediaOptions ?? null,
    });
  }
  const stem = previewStem(title, game.region);
  const multi = game.discs.length > 1;
  const options = mediaOptions ?? {
    download_boxart: true,
    download_screenshots: false,
    download_titles: false,
  };
  const media: string[] = [];
  if (options.download_boxart) media.push(`${outputDir}/${preset}/media/${stem}.png`);
  if (options.download_screenshots) media.push(`${outputDir}/${preset}/media/${stem}-screenshot.png`);
  if (options.download_titles) media.push(`${outputDir}/${preset}/media/${stem}-titlescreen.png`);
  return {
    ...game,
    canonical_title: title,
    is_multidisc: multi,
    target_m3u_path: multi ? `${outputDir}/${preset}/${stem}.m3u` : null,
    target_media_paths: media,
    discs: game.discs.map((disc) => ({
      ...disc,
      target_chd_path: multi
        ? `${outputDir}/${preset}/.discs/${stem} (Disc ${disc.disc_number}).chd`
        : `${outputDir}/${preset}/${stem}.chd`,
    })),
  };
}

export async function acceptCueRewriteApi(cuePath: string): Promise<string> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<string>('accept_cue_rewrite', { cuePath });
  }
  return cuePath;
}

export async function deployLibraryApi(sourceDir: string, destDir: string): Promise<string[]> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<string[]>('deploy_verified_library', { sourceDir, destDir });
  }
  return [];
}

let simulatedChdmanStatus: ChdmanStatus = {
  ready: false,
  source: 'missing',
  path: null,
  version: null,
};

export function _resetSimulatedChdmanStatus(status?: ChdmanStatus) {
  simulatedChdmanStatus = status || {
    ready: false,
    source: 'missing',
    path: null,
    version: null,
  };
}

export async function checkChdmanStatusApi(customPath?: string): Promise<ChdmanStatus> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<ChdmanStatus>('check_chdman_status', {
      customPath: customPath?.trim() ? customPath.trim() : null,
    });
  }

  // Browser simulation fallback
  await new Promise((r) => setTimeout(r, 80));
  if (customPath && customPath.trim()) {
    simulatedChdmanStatus = {
      ready: true,
      source: 'custom_path',
      path: customPath.trim(),
      version: '0.268',
    };
  }
  return { ...simulatedChdmanStatus };
}

export async function downloadChdmanApi(
  onProgress?: (event: DownloadProgressEvent) => void
): Promise<ChdmanStatus> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    const { listen } = await import('@tauri-apps/api/event');

    let unlisten: (() => void) | undefined;
    if (onProgress) {
      unlisten = await listen<DownloadProgressEvent>('chdman-download-progress', (e) => {
        onProgress(e.payload);
      });
    }

    try {
      return await invoke<ChdmanStatus>('download_chdman');
    } finally {
      if (unlisten) unlisten();
    }
  }

  // Browser simulation fallback with progressive byte increments
  const totalBytes = 15728640; // ~15 MB
  const milestones = [15, 35, 60, 85, 100];
  for (const p of milestones) {
    await new Promise((r) => setTimeout(r, 60));
    if (onProgress) {
      onProgress({
        downloaded_bytes: Math.round(totalBytes * (p / 100)),
        total_bytes: totalBytes,
        percentage: p,
      });
    }
  }

  simulatedChdmanStatus = {
    ready: true,
    source: 'managed_directory',
    path: 'C:/Tools/managed/chdman.exe',
    version: '0.268',
  };
  return { ...simulatedChdmanStatus };
}

export async function setCustomChdmanPathApi(path: string): Promise<ChdmanStatus> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<ChdmanStatus>('set_custom_chdman_path', { path });
  }

  // Browser simulation fallback
  await new Promise((r) => setTimeout(r, 50));
  simulatedChdmanStatus = {
    ready: true,
    source: 'custom_path',
    path,
    version: '0.268',
  };
  return { ...simulatedChdmanStatus };
}

export async function finishLibraryApi(
  plan: IngestionPlan,
  downloadArtwork: boolean,
  onProgress?: (event: ArtworkProgressEvent) => void
): Promise<FinishLibrarySummary> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    const { listen } = await import('@tauri-apps/api/event');

    let unlisten: (() => void) | undefined;
    if (onProgress) {
      unlisten = await listen<ArtworkProgressEvent>('artwork-progress', (e) => {
        onProgress(e.payload);
      });
    }

    try {
      return await invoke<FinishLibrarySummary>('finish_library', { plan, downloadArtwork });
    } finally {
      if (unlisten) unlisten();
    }
  }

  // Browser simulation: fake staggered progress then a summary.
  const games = plan.games.filter((g) => g.enabled);
  for (let i = 0; i < games.length; i++) {
    await new Promise((r) => setTimeout(r, 120));
    if (onProgress) {
      onProgress({
        game_id: games[i].id,
        title: games[i].canonical_title,
        status: i === games.length - 1 ? 'failed' : 'done',
        completed: i + 1,
        total: games.length,
      });
    }
  }
  return {
    gamelists_written: plan.preset === 'esde' || plan.preset === 'batocera' ? 1 : 0,
    artwork_downloaded: Math.max(games.length - 1, 0),
    artwork_skipped: 0,
    artwork_failed: games.length > 0 ? 1 : 0,
  };
}

export async function planMigrationApi(
  root: string,
  sourcePreset: FrontendPreset,
  targetPreset: FrontendPreset,
  customConfig?: CustomPresetConfig | null
): Promise<MigrationPlan> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<MigrationPlan>('plan_migration', {
      root,
      sourcePreset,
      targetPreset,
      customConfig: targetPreset === 'custom' ? customConfig ?? DEFAULT_CUSTOM_PRESET : null,
    });
  }
  await new Promise((r) => setTimeout(r, 300));
  return {
    root,
    source_preset: sourcePreset,
    target_preset: targetPreset,
    games: 2,
    items: [
      { kind: 'chd', source: `${root}/Roms/PS/.discs/Game (Disc 1).chd`, target: `${root}/roms/psx/.discs/Game (Disc 1).chd` },
      { kind: 'playlist', source: `${root}/Roms/PS/Game.m3u`, target: `${root}/roms/psx/Game.m3u` },
    ],
    playlist_rewrites: [],
  };
}

export async function executeMigrationApi(
  plan: MigrationPlan,
  onProgress?: (event: MigrationProgressEvent) => void
): Promise<MigrationSummary> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    const { listen } = await import('@tauri-apps/api/event');
    let unlisten: (() => void) | undefined;
    if (onProgress) {
      unlisten = await listen<MigrationProgressEvent>('migration-progress', (e) => {
        onProgress(e.payload);
      });
    }
    try {
      return await invoke<MigrationSummary>('execute_migration', { plan });
    } finally {
      if (unlisten) unlisten();
    }
  }
  await new Promise((r) => setTimeout(r, 500));
  return { files_moved: plan.items.length, playlists_rewritten: plan.playlist_rewrites.length, gamelists_written: 1, skipped_existing: [] };
}

/** redump.org system slugs for one-click DAT downloads (verified live). */
export const REDUMP_DAT_SLUGS: { platform: Platform; label: string; slug: string }[] = [
  { platform: 'psx', label: 'Sony PlayStation', slug: 'psx' },
  { platform: 'saturn', label: 'Sega Saturn', slug: 'ss' },
  { platform: 'dreamcast', label: 'Sega Dreamcast', slug: 'dc' },
  { platform: 'segacd', label: 'Sega CD / Mega-CD', slug: 'mcd' },
  { platform: 'pcecd', label: 'PC Engine CD / TurboGrafx-CD', slug: 'pce' },
];

export async function downloadRedumpDatsApi(
  destDir: string,
  slugs: string[],
  onProgress?: (message: string) => void
): Promise<string[]> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<string[]>('download_redump_dats', { destDir, slugs });
  }
  for (const slug of slugs) {
    onProgress?.(`Fetching ${slug}.dat (simulated)...`);
    await new Promise((r) => setTimeout(r, 200));
  }
  return slugs.map((s) => `${destDir}/Redump_${s}.dat`);
}

export async function readImageFileApi(path: string): Promise<string | null> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<string | null>('read_image_file', { path });
  }
  return null;
}

export async function configureWatchFolderApi(
  inputDir: string,
  outputDir: string,
  preset: FrontendPreset,
  customConfig: CustomPresetConfig | null,
  enabled: boolean
): Promise<string> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<string>('configure_watch_folder', {
      inputDir,
      outputDir,
      preset,
      customConfig: preset === 'custom' ? customConfig ?? DEFAULT_CUSTOM_PRESET : null,
      enabled,
    });
  }
  await new Promise((r) => setTimeout(r, 100));
  return enabled ? 'watching' : 'stopped';
}

export async function listenWatchStatusApi(
  onStatus: (event: WatchStatusEvent) => void
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<WatchStatusEvent>('watch-status', (e) => {
    onStatus(e.payload);
  });
  return unlisten;
}

export async function setGamePlatformApi(
  plan: IngestionPlan,
  gameId: string,
  platform: Platform
): Promise<IngestionPlan> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<IngestionPlan>('set_game_platform', { plan, gameId, platform });
  }
  // Browser simulation: swap platform and rewrite the platform folder in paths.
  const folderFor: Record<Platform, string> = {
    psx: 'roms/psx',
    saturn: 'roms/saturn',
    dreamcast: 'roms/dreamcast',
    segacd: 'roms/segacd',
    pcecd: 'roms/pcenginecd',
    unknown: 'roms/unknown',
  };
  return {
    ...plan,
    games: plan.games.map((g) =>
      g.id !== gameId
        ? g
        : {
            ...g,
            platform,
            discs: g.discs.map((d) => ({
              ...d,
              target_chd_path: d.target_chd_path.replace(/\/[^\/]+\//, '/' + folderFor[platform] + '/'),
            })),
          }
    ),
  };
}

export async function setGameTitleApi(
  plan: IngestionPlan,
  gameId: string,
  title: string
): Promise<IngestionPlan> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<IngestionPlan>('set_game_title', { plan, gameId, title });
  }
  return {
    ...plan,
    games: plan.games.map((g) => (g.id === gameId ? { ...g, canonical_title: title } : g)),
  };
}

export interface VolumeInfo {
  free_bytes: number;
  total_bytes: number;
}

export async function getVolumeInfoApi(path: string): Promise<VolumeInfo | null> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<VolumeInfo>('get_volume_info', { path });
  }
  return null;
}

export interface AppSettingsPayload {
  input_dir?: string | null;
  output_dir?: string | null;
  preset?: FrontendPreset | null;
  custom_config?: CustomPresetConfig | null;
  redump_dats?: string[] | null;
  watch_enabled?: boolean | null;
}

export async function loadAppSettings(): Promise<AppSettingsPayload | null> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<AppSettingsPayload>('get_app_settings');
  }
  return null;
}

export async function saveAppSettings(settings: AppSettingsPayload): Promise<void> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_app_settings', { settings });
  }
}

/** Native pickers (no-ops returning null in browser mode). */
export async function pickDirectory(): Promise<string | null> {
  if (isTauri()) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return (await open({ directory: true, multiple: false })) as string | null;
  }
  return null;
}

export async function pickFiles(extensions: string[]): Promise<string[] | null> {
  if (isTauri()) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const result = await open({
      multiple: true,
      filters: [{ name: extensions.join('/'), extensions }],
    });
    if (result === null) return null;
    return Array.isArray(result) ? result : [result];
  }
  return null;
}
