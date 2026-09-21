import {
  IngestionPlan,
  ExecutionSummary,
  FrontendPreset,
  JobProgressEvent,
  GameStatusEvent,
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
  apiKey?: string
): Promise<IngestionPlan> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<IngestionPlan>('scan_and_plan', {
      inputDir,
      outputDir,
      preset,
      apiKey: apiKey?.trim() ? apiKey.trim() : null,
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
          { disc_number: 1, source_descriptor: 'FF7_Disc1.cue', target_chd_path: '.multidisc/Final Fantasy VII (Disc 1).chd', status: 'pending' },
          { disc_number: 2, source_descriptor: 'FF7_Disc2.cue', target_chd_path: '.multidisc/Final Fantasy VII (Disc 2).chd', status: 'pending' },
          { disc_number: 3, source_descriptor: 'FF7_Disc3.cue', target_chd_path: '.multidisc/Final Fantasy VII (Disc 3).chd', status: 'pending' },
        ],
        target_m3u_path: 'Final Fantasy VII.m3u',
        confidence: 0.98,
        source: 'redumpcache',
        enabled: true,
        needs_review: false,
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
      },
      {
        id: 'game-mock-3',
        canonical_title: 'Panzer Dragoon Saga',
        platform: 'saturn',
        region: 'USA',
        is_multidisc: true,
        discs: [
          { disc_number: 1, source_descriptor: 'PDS_Disc1.cue', target_chd_path: '.multidisc/Panzer Dragoon Saga (Disc 1).chd', status: 'pending' },
          { disc_number: 2, source_descriptor: 'PDS_Disc2.cue', target_chd_path: '.multidisc/Panzer Dragoon Saga (Disc 2).chd', status: 'pending' },
          { disc_number: 3, source_descriptor: 'PDS_Disc3.cue', target_chd_path: '.multidisc/Panzer Dragoon Saga (Disc 3).chd', status: 'pending' },
          { disc_number: 4, source_descriptor: 'PDS_Disc4.cue', target_chd_path: '.multidisc/Panzer Dragoon Saga (Disc 4).chd', status: 'pending' },
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

export async function trashSourceFilesApi(sourceFiles: string[]): Promise<number> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<number>('trash_source_files', { sourceFiles });
  }

  await new Promise((r) => setTimeout(r, 400));
  return sourceFiles.length;
}
