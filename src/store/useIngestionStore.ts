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
} from '../types/plan';
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
} from '../services/tauri';

export interface IngestionState {
  step: 1 | 2 | 3 | 4;
  inputDir: string;
  outputDir: string;
  datPath: string;
  regionPriority: string;
  deployDest: string;
  preset: FrontendPreset;
  apiKey: string;
  mediaOptions: MediaOptions;
  plan: IngestionPlan | null;
  isScanning: boolean;
  isExecuting: boolean;
  isTrashing: boolean;
  trashedCount: number | null;
  recycledBytes: number | null;
  deployedNames: string[];
  deployError: string | null;
  gameProgress: Record<string, number>;
  discProgress: Record<string, number>;
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
  setDatPath: (path: string) => void;
  setRegionPriority: (priority: string) => void;
  setDeployDest: (path: string) => void;
  setPreset: (preset: FrontendPreset) => void;
  setApiKey: (key: string) => void;
  setMediaOptions: (options: Partial<MediaOptions>) => void;
  setError: (error: string | null) => void;
  updateGameTitle: (gameId: string, newTitle: string) => Promise<void>;
  toggleGameEnabled: (gameId: string) => void;
  setAllGamesEnabled: (enabled: boolean) => void;
  applyReleasePick: (gameId: string, title: string, region: string) => Promise<void>;
  acceptCueRewrite: (cuePath: string) => Promise<string>;
  deployLibrary: () => Promise<string[]>;
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
  datPath: '',
  regionPriority: '',
  deployDest: '',
  preset: 'anbernicstock',
  apiKey: '',
  mediaOptions: { download_boxart: true, download_screenshots: false, download_titles: false },
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
  setApiKey: (apiKey) => set({ apiKey }),
  setMediaOptions: (options) => set((state) => ({
    mediaOptions: { ...state.mediaOptions, ...options },
  })),
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

  startScan: async () => {
    const { inputDir, outputDir, preset, apiKey, mediaOptions, datPath, regionPriority } = get();
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
      const plan = await scanAndPlanApi(
        inputDir,
        outputDir,
        preset,
        apiKey,
        mediaOptions,
        datPath,
        priority
      );
      set({ plan, step: 2, isScanning: false });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ error: `Scan failed: ${msg}`, isScanning: false });
    }
  },

  startExecution: async () => {
    const { plan, isExecuting } = get();
    if (!plan || isExecuting) return;

    set({
      isExecuting: true,
      step: 3,
      error: null,
      activeLogs: [
        `[${new Date().toLocaleTimeString()}] Starting conversion engine (preset: ${plan.preset})...`,
      ],
      gameProgress: {},
  discProgress: {},
    });

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
          `[${new Date().toLocaleTimeString()}] ${finishedNote}`,
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
    const { summary } = get();
    if (!summary || summary.source_files_to_trash.length === 0) {
      return 0;
    }

    set({ isTrashing: true, error: null });
    try {
      const inputDir = get().plan?.input_dir ?? '';
      const outcome = await trashSourceFilesApi(summary.source_files_to_trash, inputDir);
      set({ trashedCount: outcome.count, recycledBytes: outcome.bytes, isTrashing: false });
      return outcome.count;
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
      recycledBytes: null,
      deployedNames: [],
      deployError: null,
      gameProgress: {},
      discProgress: {},
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
