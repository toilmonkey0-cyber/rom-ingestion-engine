import { describe, it, expect, beforeEach } from 'vitest';
import { useIngestionStore } from '../useIngestionStore';
import { IngestionPlan } from '../../types/plan';

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

describe('useIngestionStore', () => {
  beforeEach(() => {
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

  it('updates game title in plan', () => {
    useIngestionStore.setState({ plan: samplePlan });
    const store = useIngestionStore.getState();

    store.updateGameTitle('g1', 'Metal Gear Solid: Integral');
    const updated = useIngestionStore.getState();
    expect(updated.plan?.games.find((g) => g.id === 'g1')?.canonical_title).toBe(
      'Metal Gear Solid: Integral'
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
});
