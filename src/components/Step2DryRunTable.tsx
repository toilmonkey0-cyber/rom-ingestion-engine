import React, { useState, useMemo } from 'react';
import {
  ArrowLeft,
  Play,
  Search,
  CheckSquare,
  Square,
  Edit2,
  Check,
  X,
  AlertTriangle,
  CheckCircle2,
  HardDrive,
  Layers,
  Sparkles,
  Database,
  Bot,
  HelpCircle,
} from 'lucide-react';
import { PlannedGame } from '../types/plan';
import { useIngestionStore } from '../store/useIngestionStore';

function formatBytesAlready(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

const PLATFORM_OPTIONS = ['psx', 'saturn', 'dreamcast', 'segacd', 'pcecd', 'unknown'] as const;

export interface Step2DryRunTableProps {
  games?: PlannedGame[];
  onToggleGame?: (id: string) => void;
  onUpdateTitle?: (id: string, title: string) => void;
  onProceed?: () => void;
  onBack?: () => void;
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

export const Step2DryRunTable: React.FC<Step2DryRunTableProps> = ({
  games: propGames,
  onToggleGame: propToggleGame,
  onUpdateTitle: propUpdateTitle,
  onProceed: propProceed,
  onBack: propBack,
}) => {
  const store = useIngestionStore();

  const games = propGames ?? store.plan?.games ?? [];
  const toggleGame = propToggleGame ?? store.toggleGameEnabled;
  const setPlatform = store.setGamePlatform;
  const updateTitle = propUpdateTitle ?? store.updateGameTitle;
  const handleProceed = propProceed ?? store.startExecution;
  const handleBack = propBack ?? (() => store.setStep(1));

  const [searchQuery, setSearchQuery] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState('');

  const filteredGames = useMemo(() => {
    if (!searchQuery.trim()) return games;
    const q = searchQuery.toLowerCase();
    return games.filter(
      (g) =>
        g.canonical_title.toLowerCase().includes(q) ||
        g.platform.toLowerCase().includes(q) ||
        g.region.toLowerCase().includes(q)
    );
  }, [games, searchQuery]);

  const enabledCount = useMemo(() => games.filter((g) => g.enabled).length, [games]);
  const multidiscCount = useMemo(() => games.filter((g) => g.is_multidisc).length, [games]);
  const reviewCount = useMemo(() => games.filter((g) => g.needs_review || g.confidence < 0.8).length, [games]);

  const totalSourceBytes = store.plan?.total_source_bytes ?? 0;
  const estimatedOutputBytes = store.plan?.estimated_output_bytes ?? 0;
  const savedBytes = totalSourceBytes > estimatedOutputBytes ? totalSourceBytes - estimatedOutputBytes : 0;
  const savingsPct = totalSourceBytes > 0 ? Math.round((savedBytes / totalSourceBytes) * 100) : 45;

  const handleStartEdit = (game: PlannedGame) => {
    setEditingId(game.id);
    setEditingText(game.canonical_title);
  };

  const handleSaveEdit = (gameId: string) => {
    if (editingText.trim()) {
      updateTitle(gameId, editingText.trim());
      // Durable rename: re-resolves output file names on the backend.
      store.renameGame(gameId, editingText.trim());
    }
    setEditingId(null);
  };

  const handleCancelEdit = () => {
    setEditingId(null);
  };

  const handleToggleAll = () => {
    const allEnabled = games.every((g) => g.enabled);
    if (store.setAllGamesEnabled) {
      store.setAllGamesEnabled(!allEnabled);
    } else {
      games.forEach((g) => {
        if (g.enabled === allEnabled) {
          toggleGame(g.id);
        }
      });
    }
  };

  return (
    <div className="max-w-7xl mx-auto space-y-6 py-6 px-4">
      {/* Discs skipped during scanning (missing tracks, escaping references) */}
      {(store.plan?.skipped_sources?.length ?? 0) > 0 && (
        <div className="bg-amber-950/30 border border-amber-800/50 rounded-xl p-4 flex items-start space-x-3 text-xs">
          <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
          <div className="flex-1">
            <p className="font-semibold text-amber-200 mb-1">
              {store.plan!.skipped_sources!.length} disc(s) were skipped during scanning
            </p>
            <ul className="space-y-0.5 text-amber-200/70 font-mono">
              {store.plan!.skipped_sources!.slice(0, 5).map((s, i) => (
                <li key={i} className="truncate" title={s.reason}>
                  {s.path} — {s.reason}
                </li>
              ))}
              {store.plan!.skipped_sources!.length > 5 && (
                <li>…and {store.plan!.skipped_sources!.length - 5} more</li>
              )}
            </ul>
            <p className="text-amber-200/60 mt-1">
              These discs are excluded from the plan. Fix the sheets or restore the referenced tracks, then rescan.
            </p>
          </div>
        </div>
      )}

      {/* Space budget: will it fit? */}
      {(() => {
        const free = store.volumeInfo?.free_bytes ?? null;
        const estimate = store.plan?.estimated_output_bytes ?? 0;
        if (free === null) return null;
        const fits = free >= estimate;
        const short = estimate - free;
        return (
          <div
            className={`rounded-xl p-4 border flex items-start space-x-3 text-xs ${
              fits
                ? 'bg-emerald-950/30 border-emerald-800/50 text-emerald-300'
                : 'bg-red-950/40 border-red-800/60 text-red-300'
            }`}
          >
            {fits ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
            ) : (
              <AlertTriangle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
            )}
            <div>
              {fits ? (
                <span>
                  Target has <strong>{formatBytesAlready(free)}</strong> free; estimated output{' '}
                  <strong>{formatBytesAlready(estimate)}</strong> — it fits with{' '}
                  <strong>{formatBytesAlready(free - estimate)}</strong> to spare.
                </span>
              ) : (
                <span>
                  <strong>Not enough space:</strong> target has {formatBytesAlready(free)} free but the
                  plan needs ~{formatBytesAlready(estimate)} — short by{' '}
                  <strong>{formatBytesAlready(short)}</strong>. Deselect games or free space on the
                  destination.
                </span>
              )}
            </div>
          </div>
        );
      })()}

      {/* Top summary cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 flex items-center space-x-3.5 shadow-sm">
          <div className="w-10 h-10 rounded-lg bg-cyan-500/10 border border-cyan-500/20 flex items-center justify-center text-cyan-400">
            <Layers className="w-5 h-5" />
          </div>
          <div>
            <div className="text-2xl font-bold text-white tracking-tight">{games.length}</div>
            <div className="text-xs text-slate-400">Total Games Planned</div>
          </div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 flex items-center space-x-3.5 shadow-sm">
          <div className="w-10 h-10 rounded-lg bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400">
            <Sparkles className="w-5 h-5" />
          </div>
          <div>
            <div className="text-2xl font-bold text-white tracking-tight">{multidiscCount}</div>
            <div className="text-xs text-slate-400">Multi-Disc Sets (M3U)</div>
          </div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 flex items-center space-x-3.5 shadow-sm">
          <div className="w-10 h-10 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
            <HardDrive className="w-5 h-5" />
          </div>
          <div>
            <div className="text-2xl font-bold text-emerald-400 tracking-tight">
              {totalSourceBytes > 0 ? `~${savingsPct}% Saved` : '40-60% Saved'}
            </div>
            <div className="text-xs text-slate-400">
              {totalSourceBytes > 0
                ? `${formatBytes(totalSourceBytes)} → ${formatBytes(estimatedOutputBytes)}`
                : 'Lossless CHD Compression'}
            </div>
          </div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 flex items-center space-x-3.5 shadow-sm">
          <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${
            reviewCount > 0
              ? 'bg-amber-500/10 border border-amber-500/20 text-amber-400'
              : 'bg-emerald-500/10 border border-emerald-500/20 text-emerald-400'
          }`}>
            {reviewCount > 0 ? <AlertTriangle className="w-5 h-5" /> : <CheckCircle2 className="w-5 h-5" />}
          </div>
          <div>
            <div className="text-2xl font-bold text-white tracking-tight">{reviewCount}</div>
            <div className="text-xs text-slate-400">
              {reviewCount > 0 ? 'Need Review (<80%)' : 'All High Confidence'}
            </div>
          </div>
        </div>
      </div>

      {/* Table Toolbar */}
      <div className="flex flex-col sm:flex-row items-center justify-between gap-4 bg-slate-900/40 p-3 rounded-xl border border-slate-800">
        <div className="relative w-full sm:w-80">
          <Search className="w-4 h-4 text-slate-500 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search games, platforms, regions..."
            className="w-full pl-9 pr-3 py-1.5 bg-slate-950/60 border border-slate-800 rounded-lg text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
          />
        </div>

        <div className="flex items-center space-x-3 w-full sm:w-auto justify-between sm:justify-end">
          <button
            type="button"
            onClick={handleToggleAll}
            className="text-xs text-slate-400 hover:text-slate-200 transition-colors flex items-center space-x-1.5"
          >
            {games.every((g) => g.enabled) ? (
              <>
                <Square className="w-3.5 h-3.5" />
                <span>Deselect All</span>
              </>
            ) : (
              <>
                <CheckSquare className="w-3.5 h-3.5" />
                <span>Select All</span>
              </>
            )}
          </button>
          <span className="text-xs text-slate-500">|</span>
          <span className="text-xs text-cyan-400 font-medium font-mono">
            {enabledCount} of {games.length} Selected
          </span>
        </div>
      </div>

      {/* Main Interactive Table */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-xl overflow-hidden shadow-sm">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="border-b border-slate-800 bg-slate-950/40 text-slate-400 font-semibold uppercase tracking-wider text-[11px]">
                <th className="py-3 px-4 w-12 text-center">Inc</th>
                <th className="py-3 px-4 w-28">Platform</th>
                <th className="py-3 px-4 min-w-[240px]">Canonical Game Title</th>
                <th className="py-3 px-4 w-24">Region</th>
                <th className="py-3 px-4 w-28">Discs</th>
                <th className="py-3 px-4 w-32">Confidence</th>
                <th className="py-3 px-4 w-28">Source</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {filteredGames.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-8 text-center text-slate-500">
                    No games match the current query.
                  </td>
                </tr>
              ) : (
                filteredGames.map((game) => {
                  const isEditing = editingId === game.id;
                  const confidencePct = Math.round(game.confidence * 100);
                  const isLowConfidence = game.needs_review || game.confidence < 0.8;

                  return (
                    <tr
                      key={game.id}
                      className={`hover:bg-slate-800/30 transition-colors ${
                        !game.enabled ? 'opacity-40 bg-slate-950/30' : ''
                      }`}
                    >
                      {/* Inclusion Checkbox */}
                      <td className="py-3 px-4 text-center">
                        <input
                          type="checkbox"
                          checked={game.enabled}
                          onChange={() => toggleGame(game.id)}
                          aria-label={`Toggle ${game.canonical_title}`}
                          className="w-4 h-4 rounded bg-slate-900 border-slate-700 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-slate-900 cursor-pointer"
                        />
                      </td>

                      {/* Platform (editable — folder-hint misses need a manual call) */}
                      <td className="py-3 px-4 whitespace-nowrap">
                        <select
                          value={game.platform}
                          onChange={(e) => setPlatform(game.id, e.target.value as import('../types/plan').Platform)}
                          title="Platform determines the output folder"
                          className="font-mono font-semibold px-2 py-1 rounded text-[11px] bg-slate-800 text-slate-200 border border-slate-700 uppercase focus:outline-none focus:ring-2 focus:ring-cyan-500/50 cursor-pointer"
                        >
                          {PLATFORM_OPTIONS.map((p) => (
                            <option key={p} value={p}>
                              {p}
                            </option>
                          ))}
                        </select>
                      </td>

                      {/* Title & Inline Editor */}
                      <td className="py-3 px-4">
                        {isEditing ? (
                          <div className="flex items-center space-x-1.5">
                            <input
                              type="text"
                              value={editingText}
                              onChange={(e) => setEditingText(e.target.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') handleSaveEdit(game.id);
                                if (e.key === 'Escape') handleCancelEdit();
                              }}
                              autoFocus
                              className="w-full px-2 py-1 bg-slate-950 border border-cyan-500 rounded text-xs text-white focus:outline-none font-medium"
                            />
                            <button
                              type="button"
                              onClick={() => handleSaveEdit(game.id)}
                              className="p-1 rounded bg-cyan-600 hover:bg-cyan-500 text-white"
                              title="Save title"
                            >
                              <Check className="w-3.5 h-3.5" />
                            </button>
                            <button
                              type="button"
                              onClick={handleCancelEdit}
                              className="p-1 rounded bg-slate-700 hover:bg-slate-600 text-slate-300"
                              title="Cancel"
                            >
                              <X className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center space-x-2 group">
                            <span className="font-semibold text-slate-200 group-hover:text-white transition-colors">
                              {game.canonical_title}
                            </span>
                            <button
                              type="button"
                              onClick={() => handleStartEdit(game)}
                              className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-cyan-400 transition-opacity p-0.5"
                              title="Rename game"
                            >
                              <Edit2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        )}
                        {game.target_m3u_path && (
                          <div className="text-[10px] text-indigo-400 font-mono mt-0.5">
                            Playlist: {game.target_m3u_path}
                          </div>
                        )}
                      </td>

                      {/* Region */}
                      <td className="py-3 px-4 whitespace-nowrap">
                        <span className="px-2 py-0.5 rounded text-[11px] font-mono bg-slate-950 text-slate-300 border border-slate-800">
                          {game.region || 'Unknown'}
                        </span>
                      </td>

                      {/* Disc Count Badge */}
                      <td className="py-3 px-4 whitespace-nowrap">
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded text-[11px] font-medium ${
                            game.is_multidisc
                              ? 'bg-indigo-950/60 text-indigo-300 border border-indigo-800/80'
                              : 'bg-slate-800 text-slate-300'
                          }`}
                        >
                          {game.discs.length === 1 ? '1 Disc' : `${game.discs.length} Discs`}
                        </span>
                      </td>

                      {/* Confidence Badges */}
                      <td className="py-3 px-4 whitespace-nowrap">
                        <div className="flex flex-col space-y-1">
                          <div className="flex items-center space-x-1.5">
                            <span
                              className={`px-1.5 py-0.5 rounded text-[10px] font-bold font-mono ${
                                isLowConfidence
                                  ? 'bg-amber-950/80 text-amber-300 border border-amber-700/60'
                                  : 'bg-emerald-950/80 text-emerald-300 border border-emerald-700/60'
                              }`}
                            >
                              {confidencePct}%
                            </span>
                            {isLowConfidence ? (
                              <span className="text-[10px] font-semibold text-amber-400 flex items-center space-x-1">
                                <AlertTriangle className="w-3 h-3" />
                                <span>Review Needed</span>
                              </span>
                            ) : (
                              <span className="text-[10px] font-medium text-emerald-400">
                                Matched
                              </span>
                            )}
                          </div>
                        </div>
                      </td>

                      {/* Source */}
                      <td className="py-3 px-4 whitespace-nowrap">
                        <span className="inline-flex items-center space-x-1 text-[11px] text-slate-400">
                          {game.source === 'redumpcache' && (
                            <>
                              <Database className="w-3 h-3 text-cyan-400" />
                              <span>Redump</span>
                            </>
                          )}
                          {game.source === 'jevai' && (
                            <>
                              <Bot className="w-3 h-3 text-purple-400" />
                              <span>JevAI</span>
                            </>
                          )}
                          {game.source === 'fallback' && (
                            <>
                              <HelpCircle className="w-3 h-3 text-slate-400" />
                              <span>Fallback</span>
                            </>
                          )}
                        </span>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Bottom Action Controls */}
      <div className="flex items-center justify-between pt-2">
        <button
          type="button"
          onClick={handleBack}
          className="px-5 py-2.5 rounded-xl border border-slate-700 text-slate-300 hover:text-white hover:bg-slate-800 text-xs font-semibold flex items-center space-x-2 transition-all"
        >
          <ArrowLeft className="w-4 h-4" />
          <span>Back to Setup</span>
        </button>

        <button
          type="button"
          onClick={handleProceed}
          disabled={enabledCount === 0}
          className={`px-7 py-3 rounded-xl text-xs font-bold flex items-center space-x-2 transition-all shadow-lg ${
            enabledCount === 0
              ? 'bg-slate-800 text-slate-500 cursor-not-allowed border border-slate-700'
              : 'bg-gradient-to-r from-cyan-500 to-blue-600 text-slate-950 hover:brightness-110 shadow-cyan-500/20 active:scale-[0.98]'
          }`}
        >
          <Play className="w-4 h-4 fill-current" />
          <span>Start Ingestion ({enabledCount} Games)</span>
        </button>
      </div>
    </div>
  );
};
