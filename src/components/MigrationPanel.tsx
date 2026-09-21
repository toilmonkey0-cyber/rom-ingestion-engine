import React, { useState } from 'react';
import {
  ArrowRightLeft,
  Loader2,
  CheckCircle2,
  AlertTriangle,
  FolderTree,
} from 'lucide-react';
import { FrontendPreset, MigrationPlan, MigrationSummary } from '../types/plan';
import {
  planMigrationApi,
  executeMigrationApi,
} from '../services/tauri';
import { useIngestionStore } from '../store/useIngestionStore';

const PRESETS: { id: FrontendPreset; label: string }[] = [
  { id: 'anbernicstock', label: 'Anbernic Stock OS' },
  { id: 'onionos', label: 'OnionOS / GarlicOS' },
  { id: 'esde', label: 'ES-DE' },
  { id: 'batocera', label: 'Batocera / Knulli' },
  { id: 'custom', label: 'Custom' },
];

export const MigrationPanel: React.FC = () => {
  const outputDir = useIngestionStore((s) => s.outputDir);
  const preset = useIngestionStore((s) => s.preset);
  const customPresetConfig = useIngestionStore((s) => s.customPresetConfig);

  const [root, setRoot] = useState(outputDir);
  const [from, setFrom] = useState<FrontendPreset>(preset);
  const [to, setTo] = useState<FrontendPreset>('batocera');
  const [plan, setPlan] = useState<MigrationPlan | null>(null);
  const [summary, setSummary] = useState<MigrationSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handlePlan = async () => {
    setBusy(true);
    setError(null);
    setPlan(null);
    setSummary(null);
    try {
      const p = await planMigrationApi(root.trim(), from, to, customPresetConfig);
      setPlan(p);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleExecute = async () => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      const s = await executeMigrationApi(plan, (e) =>
        setProgress(`${e.completed}/${e.total} — ${e.message}`)
      );
      setSummary(s);
      setPlan(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  return (
    <details className="bg-slate-900/60 border border-slate-800 rounded-2xl shadow-sm">
      <summary className="cursor-pointer select-none p-6 pb-4 text-base font-semibold text-white flex items-center space-x-2 list-none">
        <ArrowRightLeft className="w-4 h-4 text-cyan-400" />
        <span>Re-organize an Existing Library (Preset Migration)</span>
        <span className="ml-auto text-[10px] uppercase font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-400 border border-slate-700">
          Moves only · no re-conversion
        </span>
      </summary>

      <div className="px-6 pb-6 space-y-4">
        <p className="text-xs text-slate-400 max-w-2xl">
          Switching handhelds or frontends? Point this at the organized library root (your SD card or output
          folder), pick the layout it's in and the layout you want. CHDs are frontend-agnostic, so migration
          only moves files, rewrites playlists, and regenerates metadata — nothing is re-compressed.
        </p>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="space-y-1.5 md:col-span-1">
            <label className="text-[11px] font-semibold text-slate-400 uppercase tracking-wide">
              Library Root
            </label>
            <input
              type="text"
              value={root}
              onChange={(e) => setRoot(e.target.value)}
              placeholder="e.g. E:/ or D:/Roms/Organized"
              className="w-full px-3 py-2 bg-slate-950/70 border border-slate-700/80 rounded-lg text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 font-mono"
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-[11px] font-semibold text-slate-400 uppercase tracking-wide">
              Current Layout
            </label>
            <select
              value={from}
              onChange={(e) => setFrom(e.target.value as FrontendPreset)}
              className="w-full px-3 py-2 bg-slate-950/70 border border-slate-700/80 rounded-lg text-xs text-slate-200 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
            >
              {PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-1.5">
            <label className="text-[11px] font-semibold text-slate-400 uppercase tracking-wide">
              Target Layout
            </label>
            <select
              value={to}
              onChange={(e) => setTo(e.target.value as FrontendPreset)}
              className="w-full px-3 py-2 bg-slate-950/70 border border-slate-700/80 rounded-lg text-xs text-slate-200 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
            >
              {PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="flex items-center space-x-3">
          <button
            type="button"
            onClick={handlePlan}
            disabled={busy || !root.trim() || from === to}
            className={`px-4 py-2 rounded-xl text-xs font-bold transition-all ${
              busy || !root.trim() || from === to
                ? 'bg-slate-800 text-slate-500 cursor-not-allowed border border-slate-700'
                : 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 hover:bg-cyan-500/30'
            }`}
          >
            <span className="flex items-center space-x-1.5">
              <FolderTree className="w-3.5 h-3.5" />
              <span>Plan Migration</span>
            </span>
          </button>
          {from === to && (
            <span className="text-[11px] text-amber-400/80">Pick two different layouts.</span>
          )}
        </div>

        {error && (
          <div className="bg-red-950/40 border border-red-800/60 rounded-xl p-3 text-red-300 flex items-start space-x-2 text-xs">
            <AlertTriangle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
            <span>{error}</span>
          </div>
        )}

        {plan && (
          <div className="bg-slate-950/60 border border-slate-800 rounded-xl p-4 space-y-3">
            <p className="text-xs text-slate-300">
              <strong className="text-white">{plan.games}</strong> game(s) ·{' '}
              <strong className="text-white">{plan.items.length}</strong> file move(s) planned
              {plan.playlist_rewrites.length > 0 &&
                ` · ${plan.playlist_rewrites.length} playlist(s) will be rewritten`}
            </p>
            <ul className="text-[11px] text-slate-400 font-mono space-y-1 max-h-40 overflow-y-auto">
              {plan.items.slice(0, 8).map((item, i) => (
                <li key={i} className="truncate">
                  <span className="text-cyan-400">{item.kind}</span> {item.source}
                  <span className="text-slate-600"> → </span>
                  {item.target}
                </li>
              ))}
              {plan.items.length > 8 && (
                <li className="text-slate-500">…and {plan.items.length - 8} more</li>
              )}
            </ul>
            <button
              type="button"
              onClick={handleExecute}
              disabled={busy}
              className="px-4 py-2 rounded-xl bg-amber-500/20 text-amber-300 border border-amber-500/40 hover:bg-amber-500/30 text-xs font-bold disabled:opacity-50"
            >
              Execute Migration
            </button>
          </div>
        )}

        {busy && progress && (
          <div className="flex items-center space-x-2 text-xs text-slate-400">
            <Loader2 className="w-3.5 h-3.5 animate-spin text-cyan-400" />
            <span className="font-mono truncate">{progress}</span>
          </div>
        )}

        {summary && (
          <div className="bg-emerald-950/30 border border-emerald-800/50 rounded-xl p-3 text-xs text-emerald-300 flex items-start space-x-2">
            <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
            <span>
              Migration complete: {summary.files_moved} files moved · {summary.playlists_rewritten} playlist(s)
              rewritten · {summary.gamelists_written} gamelist(s) written
              {summary.skipped_existing.length > 0 &&
                ` · ${summary.skipped_existing.length} skipped (target already existed)`}
            </span>
          </div>
        )}
      </div>
    </details>
  );
};
