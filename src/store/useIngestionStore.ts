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
} from '../types/plan';
import {
  scanAndPlanApi,
  executePlanApi,
  trashSourceFilesApi,
  checkChdmanStatusApi,
  downloadChdmanApi,
  setCustomChdmanPathApi,
  DEFAULT_CUSTOM_PRESET,
} from '../services/tauri';

export interface IngestionState {
  step: 1 | 2 | 3 | 4;
  inputDir: string;
  outputDir: string;
  preset: FrontendPreset;
  customPresetConfig: CustomPresetConfig;
  apiKey: string;
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
  setError: (error: string | null) => void;
  updateGameTitle: (gameId: string, newTitle: string) => void;
  toggleGameEnabled: (gameId: string) => void;
  setAllGamesEnabled: (enabled: boolean) => void;
  startScan: () => Promise<void>;
  startExecution: () => Promise<void>;
  trashSourceFiles: () => Promise<number>;
  checkChdmanStatus: (customPath?: string) => Promise<ChdmanStatus | null>;
  downloadChdman: () => Promise<void>;
  setCustomChdmanPath: (path: string) => Promise<void>;
  reset: () => void;
}

export const useIngestionStore = create<IngestionState>((set, get) => ({
  step: 1,
  inputDir: '',
  outputDir: '',
  preset: 'anbernicstock',
  customPresetConfig: { ...DEFAULT_CUSTOM_PRESET },
  apiKey: '',
  plan: null,
  isScanning: false,
  isExecuting: false,
  isTrashing: false,
  trashedCount: null,
  gameProgress: {},
  activeLogs: [],
  summary: null,
  error: null,
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
  setError: (error) => set({ error }),

  updateGameTitle: (gameId, newTitle) => {
    const plan = get().plan;
    if (!plan) return;
    const updatedGames = plan.games.map((game) =>
      game.id === gameId ? { ...game, canonical_title: newTitle } : game
    );
    set({ plan: { ...plan, games: updatedGames } });
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

  startScan: async () => {
    const { inputDir, outputDir, preset, apiKey, customPresetConfig } = get();
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
      const plan = await scanAndPlanApi(
        inputDir,
        outputDir,
        preset,
        apiKey,
        preset === 'custom' ? customPresetConfig : null
      );
      set({ plan, step: 2, isScanning: false });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ error: `Scan failed: ${msg}`, isScanning: false });
    }
  },

  startExecution: async () => {
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
      chdmanError: null,
      isDownloadingChdman: false,
      chdmanDownloadProgress: 0,
      chdmanDownloadedBytes: 0,
      chdmanTotalBytes: 0,
    });
  },
}));
