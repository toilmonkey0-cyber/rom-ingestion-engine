import { create } from 'zustand';
import {
  FrontendPreset,
  IngestionPlan,
  ExecutionSummary,
  JobProgressEvent,
  GameStatusEvent,
  ChdmanStatus,
  DownloadProgressEvent,
  MediaOptions,
  CustomPresetConfig,
  FinishLibrarySummary,
  ArtworkProgressEvent,
  WatchStatusEvent,
} from '../types/plan';
import type { VolumeInfo } from '../services/tauri';
import {
  scanAndPlanApi,
  executePlanApi,
  trashSourceFilesApi,
  applyDatReleaseApi,
  renamePlannedGameApi,
  acceptCueRewriteApi,
  deployLibraryApi,
  checkChdmanStatusApi,
  downloadChdmanApi,
  setCustomChdmanPathApi,
  finishLibraryApi,
  configureWatchFolderApi,
  listenWatchStatusApi,
  setGamePlatformApi,
  setGameTitleApi,
  loadAppSettings,
  saveAppSettings,
  getVolumeInfoApi,
  DEFAULT_CUSTOM_PRESET,
} from '../services/tauri';

export interface IngestionState {
  step: 1 | 2 | 3 | 4;
  inputDir: string;
  outputDir: string;
  datPath: string;
  regionPriority: string;
  deployDest: string;
  preset: FrontendPreset;
  customPresetConfig: CustomPresetConfig;
  apiKey: string;
  mediaOptions: MediaOptions;
  /** Comma/newline-separated Redump .dat paths for scan-time verification. */
  redumpDats: string;
  plan: IngestionPlan | null;
  isScanning: boolean;
  isExecuting: boolean;
  isTrashing: boolean;
  trashedCount: number | null;
  recycledBytes: number | null;
  deployedNames: string[];
  deployError: string | null;
  /** Progress keyed by `${game_id}:${disc_number}` — discs of one game run concurrently. */
  gameProgress: Record<string, number>;
  discProgress: Record<string, number>;
  activeLogs: string[];
  summary: ExecutionSummary | null;
  error: string | null;

  // Watch folder (auto-ingest)
  isWatching: boolean;
  watchStatus: WatchStatusEvent | null;

  // Finish Line (artwork + gamelist metadata)
  isFinishing: boolean;
  finishProgress: { completed: number; total: number; title: string } | null;
  finishResult: FinishLibrarySummary | null;

  // Space budgeting (target volume)
  volumeInfo: VolumeInfo | null;

  // chdman readiness and downloader state
  chdmanStatus: ChdmanStatus | null;
  isDownloadingChdman: boolean;
  chdmanDownloadProgress: number;
  chdmanDownloadedBytes: number;
  chdmanTotalBytes: number;
  chdmanError: string | null;

  // Actions
  setStep: (step: 1 | 2 | 3 | 4) => void;
  setInputDir: (dir: string) => void;
  setOutputDir: (dir: string) => void;
  setDatPath: (path: string) => void;
  setRegionPriority: (priority: string) => void;
  setDeployDest: (path: string) => void;
  setPreset: (preset: FrontendPreset) => void;
  setCustomPresetFolder: (field: keyof CustomPresetConfig, value: string) => void;
  setApiKey: (key: string) => void;
  setMediaOptions: (options: Partial<MediaOptions>) => void;
  setRedumpDats: (dats: string) => void;
  configureWatch: (enabled: boolean) => Promise<void>;
  hydrateFromSettings: () => Promise<void>;
  refreshVolumeInfo: () => Promise<void>;
  setError: (error: string | null) => void;
  updateGameTitle: (gameId: string, newTitle: string) => Promise<void>;
  toggleGameEnabled: (gameId: string) => void;
  setAllGamesEnabled: (enabled: boolean) => void;
  applyReleasePick: (gameId: string, title: string, region: string) => Promise<void>;
  acceptCueRewrite: (cuePath: string) => Promise<string>;
  deployLibrary: () => Promise<string[]>;
  setGamePlatform: (gameId: string, platform: import('../types/plan').Platform) => Promise<void>;
  renameGame: (gameId: string, title: string) => Promise<void>;
  startScan: () => Promise<void>;
  startExecution: () => Promise<void>;
  trashSourceFiles: (confirmPermanent?: boolean) => Promise<number>;
  finishLibrary: (downloadArtwork: boolean) => Promise<FinishLibrarySummary | null>;
  checkChdmanStatus: (customPath?: string) => Promise<ChdmanStatus | null>;
  downloadChdman: () => Promise<void>;
  setCustomChdmanPath: (path: string) => Promise<void>;
  reset: () => void;
}

// Serializes plan mutations (title/platform edits) so execution never
// captures a plan that is mid-update: startExecution awaits this chain first.
let planOpsChain: Promise<void> = Promise.resolve();

export const useIngestionStore = create<IngestionState>((set, get) => ({
  step: 1,
  inputDir: '',
  outputDir: '',
  datPath: '',
  regionPriority: '',
  deployDest: '',
  preset: 'anbernicstock',
  customPresetConfig: { ...DEFAULT_CUSTOM_PRESET },
  apiKey: '',
  mediaOptions: { download_boxart: true, download_screenshots: false, download_titles: false },
  redumpDats: '',
  plan: null,
  isScanning: false,
  isExecuting: false,
  isTrashing: false,
  trashedCount: null,
  recycledBytes: null,
  deployedNames: [],
  deployError: null,
  gameProgress: {},
  discProgress: {},
  activeLogs: [],
  summary: null,
  error: null,
  isWatching: false,
  watchStatus: null,
  isFinishing: false,
  finishProgress: null,
  finishResult: null,
  volumeInfo: null,
  chdmanStatus: null,
  isDownloadingChdman: false,
  chdmanDownloadProgress: 0,
  chdmanDownloadedBytes: 0,
  chdmanTotalBytes: 0,
  chdmanError: null,

  setStep: (step) => set({ step }),
  setInputDir: (inputDir) => set({ inputDir }),
  setOutputDir: (outputDir) => set({ outputDir }),
  setDatPath: (datPath) => set({ datPath }),
  setRegionPriority: (regionPriority) => set({ regionPriority }),
  setDeployDest: (deployDest) => set({ deployDest }),
  setPreset: (preset) => set({ preset }),
  setCustomPresetFolder: (field, value) =>
    set((state) => ({
      customPresetConfig: { ...state.customPresetConfig, [field]: value },
    })),
  setApiKey: (apiKey) => set({ apiKey }),
  setMediaOptions: (options) => set((state) => ({
    mediaOptions: { ...state.mediaOptions, ...options },
  })),
  setRedumpDats: (redumpDats) => set({ redumpDats }),

  configureWatch: async (enabled: boolean) => {
    const { inputDir, outputDir, preset, customPresetConfig } = get();
    if (enabled && (!inputDir.trim() || !outputDir.trim())) {
      set({ error: 'Set the source and target folders before enabling the watch folder.' });
      return;
    }
    try {
      const status = await configureWatchFolderApi(
        inputDir,
        outputDir,
        preset,
        preset === 'custom' ? customPresetConfig : null,
        enabled
      );
      set({
        isWatching: status === 'watching',
        watchStatus: enabled
          ? { stage: 'watching', message: 'Watching for new dumps...' }
          : null,
        error: null,
      });
      if (enabled) {
        listenWatchStatusApi((event) => set({ watchStatus: event })).catch(() => {});
      }
    } catch (err: unknown) {
      set({ error: err instanceof Error ? err.message : String(err), isWatching: false });
    }
  },
  setError: (error) => set({ error }),

  updateGameTitle: async (gameId, newTitle) => {
    const trimmed = newTitle.trim();
    const { plan, outputDir, preset, mediaOptions } = get();
    if (!plan || !trimmed) return;
    const game = plan.games.find((item) => item.id === gameId);
    if (!game) return;
    const updated = await renamePlannedGameApi(
      game,
      trimmed,
      outputDir || plan.output_dir,
      preset,
      mediaOptions
    );
    const current = get().plan;
    if (!current) return;
    set({
      plan: {
        ...current,
        games: current.games.map((item) => (item.id === gameId ? updated : item)),
      },
    });
  },

  renameGame: async (gameId, title) => {
    const run = async () => {
      const plan = get().plan;
      if (!plan) return;
      try {
        const updated = await setGameTitleApi(plan, gameId, title);
        set({ plan: updated, error: null });
      } catch (err: unknown) {
        set({ error: err instanceof Error ? err.message : String(err) });
      }
    };
    planOpsChain = planOpsChain.then(run, run);
    await planOpsChain;
  },

  toggleGameEnabled: (gameId) => {
    const plan = get().plan;
    if (!plan) return;
    const updatedGames = plan.games.map((game) =>
      game.id === gameId ? { ...game, enabled: !game.enabled } : game
    );
    set({ plan: { ...plan, games: updatedGames } });
  },

  setAllGamesEnabled: (enabled) => {
    const plan = get().plan;
    if (!plan) return;
    const updatedGames = plan.games.map((game) => ({ ...game, enabled }));
    set({ plan: { ...plan, games: updatedGames } });
  },

  applyReleasePick: async (gameId, title, region) => {
    const { plan, outputDir, preset } = get();
    if (!plan) return;
    const game = plan.games.find((item) => item.id === gameId);
    if (!game || !title.trim() || !region.trim()) return;
    const updated = await applyDatReleaseApi(
      game,
      title.trim(),
      region.trim(),
      outputDir || plan.output_dir,
      preset
    );
    set({
      plan: {
        ...plan,
        games: plan.games.map((item) => (item.id === gameId ? updated : item)),
      },
    });
  },

  acceptCueRewrite: async (cuePath) => {
    const rewritten = await acceptCueRewriteApi(cuePath);
    const plan = get().plan;
    if (!plan) return rewritten;
    set({
      plan: {
        ...plan,
        games: plan.games.map((game) =>
          game.discs.some((disc) => disc.source_descriptor === cuePath)
            ? { ...game, status_note: `Cue rewritten: ${rewritten}` }
            : game
        ),
      },
    });
    return rewritten;
  },

  deployLibrary: async () => {
    const { outputDir, deployDest, plan } = get();
    const source = outputDir.trim() || plan?.output_dir || '';
    if (!source || !deployDest.trim()) {
      set({ deployError: 'Choose a library folder and a copy destination.' });
      return [];
    }
    try {
      const names = await deployLibraryApi(source, deployDest.trim());
      set({ deployedNames: names, deployError: null });
      return names;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ deployError: msg, deployedNames: [] });
      throw err;
    }
  },

  setGamePlatform: async (gameId, platform) => {
    const run = async () => {
      const plan = get().plan;
      if (!plan) return;
      try {
        const updated = await setGamePlatformApi(plan, gameId, platform);
        set({ plan: updated, error: null });
      } catch (err: unknown) {
        set({ error: err instanceof Error ? err.message : String(err) });
      }
    };
    planOpsChain = planOpsChain.then(run, run);
    await planOpsChain;
  },

  hydrateFromSettings: async () => {
    const saved = await loadAppSettings();
    if (!saved) return;
    const state = get();
    set({
      inputDir: state.inputDir || saved.input_dir || '',
      outputDir: state.outputDir || saved.output_dir || '',
      preset: saved.preset ?? state.preset,
      customPresetConfig: saved.custom_config ?? state.customPresetConfig,
      redumpDats: state.redumpDats || (saved.redump_dats ?? []).join(', '),
    });
  },

  refreshVolumeInfo: async () => {
    const { outputDir } = get();
    if (!outputDir.trim()) {
      set({ volumeInfo: null });
      return;
    }
    try {
      const info = await getVolumeInfoApi(outputDir.trim());
      set({ volumeInfo: info });
    } catch {
      set({ volumeInfo: null });
    }
  },

  startScan: async () => {
    const { inputDir, outputDir, preset, apiKey, mediaOptions, datPath, regionPriority, customPresetConfig, redumpDats } = get();
    if (!inputDir.trim()) {
      set({ error: 'Please specify an input folder with your disc dumps.' });
      return;
    }
    if (!outputDir.trim()) {
      set({ error: 'Please specify an output destination folder.' });
      return;
    }

    const priority = regionPriority
      .split(',')
      .map((region) => region.trim())
      .filter((region) => region.length > 0);

    set({ isScanning: true, error: null });
    try {
      const datPaths = redumpDats
        .split(/[,;\n]/)
        .map((p) => p.trim())
        .filter(Boolean);
      const plan = await scanAndPlanApi(
        inputDir,
        outputDir,
        preset,
        apiKey,
        mediaOptions,
        datPath,
        priority,
        preset === 'custom' ? customPresetConfig : null,
        datPaths.length > 0 ? datPaths : null
      );
      set({ plan, step: 2, isScanning: false });
      get().refreshVolumeInfo();
      saveAppSettings({
        input_dir: inputDir,
        output_dir: outputDir,
        preset,
        custom_config: preset === 'custom' ? customPresetConfig : null,
        redump_dats: datPaths.length > 0 ? datPaths : null,
      }).catch(() => {});
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ error: `Scan failed: ${msg}`, isScanning: false });
    }
  },

  startExecution: async () => {
    await planOpsChain; // never execute a plan that is mid-mutation
    const { plan, isExecuting } = get();
    if (!plan) return;
    if (isExecuting) return; // reentrancy guard: never run two pipelines at once

    set({
      isExecuting: true,
      step: 3,
      error: null,
      summary: null,
      activeLogs: [
        `[${new Date().toLocaleTimeString()}] Starting conversion engine (preset: ${plan.preset})...`,
      ],
      gameProgress: {},
  discProgress: {},
    });

    // Discs of one game run concurrently on the backend, so progress is keyed
    // per disc and averaged per game when displayed.
    const onProgress = (event: JobProgressEvent) => {
      const log = `[${new Date().toLocaleTimeString()}] [Disc ${event.disc_number}] ${event.message}`;
      set((state) => ({
        gameProgress: { ...state.gameProgress, [event.game_id]: event.progress },
        discProgress: {
          ...state.discProgress,
          [`${event.game_id}:${event.disc_number}`]: event.progress,
        },
        activeLogs: [...state.activeLogs.slice(-150), log],
      }));
    };

    const onStatus = (event: GameStatusEvent) => {
      const log = event.error
        ? `[${new Date().toLocaleTimeString()}] ERROR (${event.game_id}): ${event.error}`
        : `[${new Date().toLocaleTimeString()}] Game ${event.game_id} status: ${event.status}`;
      set((state) => ({
        activeLogs: [...state.activeLogs.slice(-150), log],
      }));
    };

    try {
      const summary = await executePlanApi(plan, onProgress, onStatus);
      const finishedNote =
        summary.failed_games > 0
          ? `Ingestion finished with ${summary.failed_games} failed game(s).`
          : 'Ingestion finished.';
      set({
        summary,
        isExecuting: false,
        step: 4,
        activeLogs: [
          ...get().activeLogs,
          `[${new Date().toLocaleTimeString()}] ${finishedNote} (${summary.successful_games} succeeded)`,
        ],
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({
        error: `Execution error: ${msg}`,
        isExecuting: false,
        activeLogs: [
          ...get().activeLogs,
          `[${new Date().toLocaleTimeString()}] Pipeline aborted: ${msg}`,
        ],
      });
    }
  },

  trashSourceFiles: async (confirmPermanent = false) => {
    const { summary, plan } = get();
    if (!summary || summary.source_files_to_trash.length === 0) {
      return 0;
    }

    set({ isTrashing: true, error: null });
    try {
      const outcome = await trashSourceFilesApi(
        summary.source_files_to_trash,
        plan?.input_dir ?? null,
        confirmPermanent
      );
      set({ trashedCount: outcome.count, recycledBytes: outcome.bytes, isTrashing: false });
      return outcome.count;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ isTrashing: false });
      // The no-Recycle-Bin refusal is not an app failure: rethrow so the UI
      // can offer the explicit permanent-delete confirmation.
      throw err instanceof Error ? err : new Error(msg);
    }
  },

  finishLibrary: async (downloadArtwork: boolean) => {
    const { plan, isFinishing } = get();
    if (!plan || isFinishing) return null;

    set({ isFinishing: true, error: null, finishResult: null, finishProgress: null });
    const onProgress = (event: ArtworkProgressEvent) => {
      set({
        finishProgress: { completed: event.completed, total: event.total, title: event.title },
      });
    };

    try {
      const result = await finishLibraryApi(plan, downloadArtwork, onProgress);
      set({ finishResult: result, isFinishing: false });
      return result;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ error: `Finish Line failed: ${msg}`, isFinishing: false });
      return null;
    }
  },

  checkChdmanStatus: async (customPath?: string) => {
    try {
      const status = await checkChdmanStatusApi(customPath);
      set({ chdmanStatus: status, chdmanError: null });
      return status;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ chdmanError: `chdman check failed: ${msg}` });
      return null;
    }
  },

  downloadChdman: async () => {
    set({
      isDownloadingChdman: true,
      chdmanDownloadProgress: 0,
      chdmanDownloadedBytes: 0,
      chdmanTotalBytes: 0,
      chdmanError: null,
    });

    const onProgress = (event: DownloadProgressEvent) => {
      set({
        chdmanDownloadProgress: event.percentage,
        chdmanDownloadedBytes: event.downloaded_bytes,
        chdmanTotalBytes: event.total_bytes,
      });
    };

    try {
      const status = await downloadChdmanApi(onProgress);
      set({
        chdmanStatus: status,
        isDownloadingChdman: false,
        chdmanDownloadProgress: 100,
        chdmanError: null,
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({
        chdmanError: `Download failed: ${msg}`,
        isDownloadingChdman: false,
      });
    }
  },

  setCustomChdmanPath: async (path: string) => {
    if (!path.trim()) return;
    try {
      const status = await setCustomChdmanPathApi(path.trim());
      set({ chdmanStatus: status, chdmanError: null });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ chdmanError: `Invalid chdman binary: ${msg}` });
    }
  },

  reset: () => {
    set({
      step: 1,
      plan: null,
      isScanning: false,
      isExecuting: false,
      isTrashing: false,
      trashedCount: null,
      recycledBytes: null,
      deployedNames: [],
      deployError: null,
      gameProgress: {},
      discProgress: {},
      activeLogs: [],
      summary: null,
      error: null,
      isWatching: false,
      watchStatus: null,
      isFinishing: false,
      finishProgress: null,
      finishResult: null,
      chdmanError: null,
      isDownloadingChdman: false,
      chdmanDownloadProgress: 0,
      chdmanDownloadedBytes: 0,
      chdmanTotalBytes: 0,
    });
  },
}));
