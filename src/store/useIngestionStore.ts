import { create } from 'zustand';
import {
  FrontendPreset,
  IngestionPlan,
  ExecutionSummary,
  JobProgressEvent,
  GameStatusEvent,
  ChdmanStatus,
  DownloadProgressEvent,
  CustomPresetConfig,
  FinishLibrarySummary,
  ArtworkProgressEvent,
  WatchStatusEvent,
} from '../types/plan';
import {
  scanAndPlanApi,
  executePlanApi,
  trashSourceFilesApi,
  checkChdmanStatusApi,
  downloadChdmanApi,
  setCustomChdmanPathApi,
  finishLibraryApi,
  configureWatchFolderApi,
  listenWatchStatusApi,
  setGamePlatformApi,
  setGameTitleApi,
  DEFAULT_CUSTOM_PRESET,
} from '../services/tauri';

export interface IngestionState {
  step: 1 | 2 | 3 | 4;
  inputDir: string;
  outputDir: string;
  preset: FrontendPreset;
  customPresetConfig: CustomPresetConfig;
  apiKey: string;
  /** Comma/newline-separated Redump .dat paths for scan-time verification. */
  redumpDats: string;
  plan: IngestionPlan | null;
  isScanning: boolean;
  isExecuting: boolean;
  isTrashing: boolean;
  trashedCount: number | null;
  /** Progress keyed by `${game_id}:${disc_number}` — discs of one game run concurrently. */
  gameProgress: Record<string, number>;
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
  setPreset: (preset: FrontendPreset) => void;
  setCustomPresetFolder: (field: keyof CustomPresetConfig, value: string) => void;
  setApiKey: (key: string) => void;
  setRedumpDats: (dats: string) => void;
  configureWatch: (enabled: boolean) => Promise<void>;
  setError: (error: string | null) => void;
  updateGameTitle: (gameId: string, newTitle: string) => void;
  toggleGameEnabled: (gameId: string) => void;
  setAllGamesEnabled: (enabled: boolean) => void;
  setGamePlatform: (gameId: string, platform: import('../types/plan').Platform) => Promise<void>;
  renameGame: (gameId: string, title: string) => Promise<void>;
  startScan: () => Promise<void>;
  startExecution: () => Promise<void>;
  trashSourceFiles: () => Promise<number>;
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
  preset: 'anbernicstock',
  customPresetConfig: { ...DEFAULT_CUSTOM_PRESET },
  apiKey: '',
  redumpDats: '',
  plan: null,
  isScanning: false,
  isExecuting: false,
  isTrashing: false,
  trashedCount: null,
  gameProgress: {},
  activeLogs: [],
  summary: null,
  error: null,
  isWatching: false,
  watchStatus: null,
  isFinishing: false,
  finishProgress: null,
  finishResult: null,
  chdmanStatus: null,
  isDownloadingChdman: false,
  chdmanDownloadProgress: 0,
  chdmanDownloadedBytes: 0,
  chdmanTotalBytes: 0,
  chdmanError: null,

  setStep: (step) => set({ step }),
  setInputDir: (inputDir) => set({ inputDir }),
  setOutputDir: (outputDir) => set({ outputDir }),
  setPreset: (preset) => set({ preset }),
  setCustomPresetFolder: (field, value) =>
    set((state) => ({
      customPresetConfig: { ...state.customPresetConfig, [field]: value },
    })),
  setApiKey: (apiKey) => set({ apiKey }),
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

  updateGameTitle: (gameId, newTitle) => {
    // Instant display-only echo; the durable rename (with re-resolved file
    // names) goes through renameGame below.
    const plan = get().plan;
    if (!plan) return;
    const updatedGames = plan.games.map((game) =>
      game.id === gameId ? { ...game, canonical_title: newTitle } : game
    );
    set({ plan: { ...plan, games: updatedGames } });
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

  startScan: async () => {
    const { inputDir, outputDir, preset, apiKey, customPresetConfig, redumpDats } = get();
    if (!inputDir.trim()) {
      set({ error: 'Please specify an input folder with your disc dumps.' });
      return;
    }
    if (!outputDir.trim()) {
      set({ error: 'Please specify an output destination folder.' });
      return;
    }

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
        preset === 'custom' ? customPresetConfig : null,
        datPaths.length > 0 ? datPaths : null
      );
      set({ plan, step: 2, isScanning: false });
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
    });

    // Discs of one game run concurrently on the backend, so progress is keyed
    // per disc and averaged per game when displayed.
    const onProgress = (event: JobProgressEvent) => {
      const log = `[${new Date().toLocaleTimeString()}] [Disc ${event.disc_number}] ${event.message}`;
      set((state) => ({
        gameProgress: {
          ...state.gameProgress,
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
      set({
        summary,
        isExecuting: false,
        step: 4,
        activeLogs: [
          ...get().activeLogs,
          `[${new Date().toLocaleTimeString()}] Ingestion pipeline finished: ${summary.successful_games} succeeded, ${summary.failed_games} failed.`,
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

  trashSourceFiles: async () => {
    const { summary, plan } = get();
    if (!summary || summary.source_files_to_trash.length === 0) {
      return 0;
    }

    set({ isTrashing: true, error: null });
    try {
      const count = await trashSourceFilesApi(
        summary.source_files_to_trash,
        plan?.input_dir ?? null
      );
      set({ trashedCount: count, isTrashing: false });
      return count;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ error: `Trash operation failed: ${msg}`, isTrashing: false });
      throw err;
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
      gameProgress: {},
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
