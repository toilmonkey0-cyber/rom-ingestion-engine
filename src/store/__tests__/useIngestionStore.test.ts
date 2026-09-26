import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useIngestionStore } from '../useIngestionStore';
import { ExecutionSummary, IngestionPlan } from '../../types/plan';

const executePlanApi = vi.hoisted(() => vi.fn());
const scanAndPlanApi = vi.hoisted(() => vi.fn());

vi.mock('../../services/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../services/tauri')>();
  return {
    ...actual,
    executePlanApi: (...args: unknown[]) => executePlanApi(...args),
    scanAndPlanApi: (...args: unknown[]) => scanAndPlanApi(...args),
  };
});

const samplePlan: IngestionPlan = {
  input_dir: 'D:/In',
  output_dir: 'D:/Out',
  preset: 'anbernicstock',
  total_source_bytes: 1000,
  estimated_output_bytes: 600,
  games: [
    {
      id: 'g1',
      canonical_title: 'Metal Gear Solid',
      platform: 'psx',
      region: 'USA',
      is_multidisc: true,
      discs: [
        { disc_number: 1, source_descriptor: 'MGS1.cue', target_chd_path: 'MGS1.chd', status: 'pending' },
        { disc_number: 2, source_descriptor: 'MGS2.cue', target_chd_path: 'MGS2.chd', status: 'pending' },
      ],
      target_m3u_path: 'Metal Gear Solid.m3u',
      confidence: 0.96,
      source: 'redumpcache',
      enabled: true,
      needs_review: false,
    },
    {
      id: 'g2',
      canonical_title: 'Tekken 3',
      platform: 'psx',
      region: 'USA',
      is_multidisc: false,
      discs: [
        { disc_number: 1, source_descriptor: 'Tekken3.cue', target_chd_path: 'Tekken 3.chd', status: 'pending' },
      ],
      target_m3u_path: null,
      confidence: 0.99,
      source: 'redumpcache',
      enabled: true,
      needs_review: false,
    },
  ],
};

const quietSummary: ExecutionSummary = {
  total_games: 1,
  successful_games: 1,
  failed_games: 0,
  total_discs: 1,
  processed_discs: 1,
  source_files_to_trash: [],
  total_source_bytes: 1,
  total_output_bytes: 1,
};

describe('useIngestionStore', () => {
  beforeEach(() => {
    executePlanApi.mockReset();
    scanAndPlanApi.mockReset();
    scanAndPlanApi.mockResolvedValue(samplePlan);
    useIngestionStore.getState().reset();
  });

  it('updates configuration fields correctly', () => {
    const store = useIngestionStore.getState();
    store.setInputDir('E:/Dumps');
    store.setOutputDir('F:/Roms');
    store.setPreset('esde');
    store.setApiKey('test-key');

    const updated = useIngestionStore.getState();
    expect(updated.inputDir).toBe('E:/Dumps');
    expect(updated.outputDir).toBe('F:/Roms');
    expect(updated.preset).toBe('esde');
    expect(updated.apiKey).toBe('test-key');
  });

  it('updates game title in plan', async () => {
    useIngestionStore.setState({ plan: samplePlan, outputDir: 'D:/Out', preset: 'esde' });
    await useIngestionStore.getState().updateGameTitle('g1', 'Metal Gear Solid: Integral');
    const game = useIngestionStore.getState().plan?.games.find((item) => item.id === 'g1');
    expect(game?.canonical_title).toBe('Metal Gear Solid: Integral');
    expect(game?.discs[0].target_chd_path).toBe(
      'D:/Out/esde/.discs/Metal Gear Solid_ Integral (USA) (Disc 1).chd'
    );
    expect(game?.target_m3u_path).toBe('D:/Out/esde/Metal Gear Solid_ Integral (USA).m3u');
    expect(game?.target_media_paths?.[0]).toBe(
      'D:/Out/esde/media/Metal Gear Solid_ Integral (USA).png'
    );
  });

  it('toggles game enabled state', () => {
    useIngestionStore.setState({ plan: samplePlan });
    const store = useIngestionStore.getState();

    store.toggleGameEnabled('g1');
    let game = useIngestionStore.getState().plan?.games.find((g) => g.id === 'g1');
    expect(game?.enabled).toBe(false);

    store.toggleGameEnabled('g1');
    game = useIngestionStore.getState().plan?.games.find((g) => g.id === 'g1');
    expect(game?.enabled).toBe(true);
  });

  it('enables and disables all games in bulk', () => {
    useIngestionStore.setState({ plan: samplePlan });
    const store = useIngestionStore.getState();

    store.setAllGamesEnabled(false);
    expect(useIngestionStore.getState().plan?.games.every((g) => !g.enabled)).toBe(true);

    store.setAllGamesEnabled(true);
    expect(useIngestionStore.getState().plan?.games.every((g) => g.enabled)).toBe(true);
  });

  it('ignores a second startExecution while one is running', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    executePlanApi.mockImplementation(async () => {
      await gate;
      return quietSummary;
    });

    useIngestionStore.setState({ plan: samplePlan });
    const first = useIngestionStore.getState().startExecution();
    const second = useIngestionStore.getState().startExecution();
    await second;

    expect(executePlanApi).toHaveBeenCalledTimes(1);
    expect(useIngestionStore.getState().isExecuting).toBe(true);

    release();
    await first;
    expect(useIngestionStore.getState().isExecuting).toBe(false);
    expect(useIngestionStore.getState().activeLogs.some((line) => line.includes('Ingestion finished.'))).toBe(true);
  });

  it('checks chdman status and updates store state', async () => {
    const store = useIngestionStore.getState();
    const status = await store.checkChdmanStatus();

    expect(status).toBeDefined();
    expect(useIngestionStore.getState().chdmanStatus).toEqual(status);
  });

  it('downloads chdman with progress updates and marks ready', async () => {
    const store = useIngestionStore.getState();
    await store.downloadChdman();

    const updated = useIngestionStore.getState();
    expect(updated.isDownloadingChdman).toBe(false);
    expect(updated.chdmanDownloadProgress).toBe(100);
    expect(updated.chdmanStatus?.ready).toBe(true);
    expect(updated.chdmanStatus?.source).toBe('managed_directory');
  });

  it('sends the user DAT and region priority when scanning', async () => {
    useIngestionStore.setState({
      inputDir: 'D:/In',
      outputDir: 'D:/Out',
      preset: 'anbernicstock',
      apiKey: '',
      datPath: 'D:/dats/psx.dat',
      regionPriority: 'USA, Japan',
    });
    await useIngestionStore.getState().startScan();
    expect(scanAndPlanApi).toHaveBeenCalledWith(
      'D:/In',
      'D:/Out',
      'anbernicstock',
      '',
      { download_boxart: true, download_screenshots: false, download_titles: false },
      'D:/dats/psx.dat',
      ['USA', 'Japan']
    );
  });

  it('replaces the shown CHD path when a DAT release is picked', async () => {
    useIngestionStore.setState({ plan: samplePlan, outputDir: 'D:/Out', preset: 'esde' });
    await useIngestionStore.getState().applyReleasePick('g2', 'Tekken 3 Director', 'Japan');
    const game = useIngestionStore.getState().plan?.games.find((item) => item.id === 'g2');
    expect(game?.canonical_title).toBe('Tekken 3 Director');
    expect(game?.region).toBe('Japan');
    expect(game?.discs[0].target_chd_path).toBe('D:/Out/esde/Tekken 3 Director (Japan).chd');
  });

  it('records recycled bytes from the trash outcome', async () => {
    useIngestionStore.setState({
      plan: samplePlan,
      summary: {
        ...quietSummary,
        source_files_to_trash: ['a.cue', 'a.bin'],
      },
    });
    const count = await useIngestionStore.getState().trashSourceFiles();
    expect(count).toBe(2);
    expect(useIngestionStore.getState().trashedCount).toBe(2);
    expect(useIngestionStore.getState().recycledBytes).toBe(2);
  });

  it('sets custom chdman path and marks ready', async () => {
    const store = useIngestionStore.getState();
    await store.setCustomChdmanPath('D:/Custom/chdman.exe');

    const updated = useIngestionStore.getState();
    expect(updated.chdmanStatus?.ready).toBe(true);
    expect(updated.chdmanStatus?.source).toBe('custom_path');
    expect(updated.chdmanStatus?.path).toBe('D:/Custom/chdman.exe');
  });
});
