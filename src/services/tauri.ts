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
} from '../types/plan';

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
  regionPriority?: string[]
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

export async function trashSourceFilesApi(sourceFiles: string[], inputDir: string): Promise<TrashOutcome> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<TrashOutcome>('trash_source_files', { sourceFiles, inputDir });
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
