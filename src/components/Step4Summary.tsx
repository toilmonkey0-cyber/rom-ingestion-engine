import React, { useState } from 'react';
import {
  ShieldCheck,
  HardDrive,
  Trash2,
  RotateCcw,
  CheckCircle2,
  AlertTriangle,
  FolderSync,
  FileCheck,
  Layers,
  X,
  Sparkles,
} from 'lucide-react';
import { useIngestionStore } from '../store/useIngestionStore';

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

export const Step4Summary: React.FC = () => {
  const {
    summary,
    isTrashing,
    trashedCount,
    recycledBytes,
    trashSourceFiles,
    reset,
    error,
    outputDir,
    deployDest,
    deployedNames,
    deployError,
    setDeployDest,
    deployLibrary,
  } = useIngestionStore();

  const [showTrashModal, setShowTrashModal] = useState(false);

  const totalSource = summary?.total_source_bytes ?? 0;
  const totalOutput = summary?.total_output_bytes ?? 0;
  const failedGames = summary?.failed_games ?? 0;
  const successfulGames = summary?.successful_games ?? 0;
  const runFailed = failedGames > 0;

  const filesToTrash = summary?.source_files_to_trash ?? [];
  const hasFilesToTrash = filesToTrash.length > 0 && trashedCount === null;
  const trashLocked = runFailed || !hasFilesToTrash || isTrashing;
  const failedIds = summary?.failed_game_ids ?? [];
  const partialIds = summary?.partial_game_ids ?? [];

  const handleConfirmTrash = async () => {
    try {
      await trashSourceFiles();
      setShowTrashModal(false);
    } catch {
      // Error handled by store
    }
  };

  return (
    <div className="max-w-4xl mx-auto space-y-8 py-6 px-4">
      <div
        className={`rounded-2xl p-6 shadow-2xl relative overflow-hidden border ${
          runFailed
            ? 'bg-gradient-to-r from-red-950/50 via-slate-900 to-slate-900 border-red-800/60'
            : 'bg-gradient-to-r from-emerald-950/40 via-slate-900 to-slate-900 border-emerald-800/50'
        }`}
      >
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-6">
          <div className="flex items-center space-x-4">
            <div
              className={`w-14 h-14 rounded-2xl flex items-center justify-center shrink-0 ${
                runFailed
                  ? 'bg-red-500/10 border border-red-500/30 text-red-400'
                  : 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-400'
              }`}
            >
              {runFailed ? <AlertTriangle className="w-8 h-8" /> : <ShieldCheck className="w-8 h-8" />}
            </div>
            <div>
              <h2 className="text-2xl font-bold text-white tracking-tight">
                {runFailed ? 'Ingestion finished with failures' : 'Ingestion finished'}
              </h2>
              <p className="text-xs text-slate-300 mt-1 max-w-lg">
                {runFailed
                  ? `${failedGames} game(s) failed and ${successfulGames} succeeded. Output written: ${formatBytes(totalOutput)}. Source dumps stay in place until a run finishes with no failures.`
                  : `${successfulGames} game(s) written. Output size ${formatBytes(totalOutput)}. Each CHD was accepted after a header check.`}
              </p>
            </div>
          </div>

          <div
            className={`rounded-xl px-5 py-3 text-center self-start sm:self-auto shrink-0 border ${
              runFailed
                ? 'bg-red-950/60 border-red-800/80'
                : 'bg-emerald-950/60 border-emerald-800/80'
            }`}
          >
            <div className={`text-2xl font-black font-mono ${runFailed ? 'text-red-300' : 'text-emerald-400'}`}>
              {runFailed ? failedGames : formatBytes(totalOutput)}
            </div>
            <div className={`text-[11px] font-medium ${runFailed ? 'text-red-200/80' : 'text-emerald-200/80'}`}>
              {runFailed ? 'Games Failed' : 'Output Written'}
            </div>
          </div>
        </div>
      </div>

      {error && (
        <div className="bg-red-950/40 border border-red-800/60 rounded-xl p-4 text-red-300 flex items-start space-x-3 text-xs">
          <AlertTriangle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
          <div className="flex-1">{error}</div>
        </div>
      )}

      {/* Metrics Grid */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <FileCheck className="w-5 h-5 text-cyan-400 mx-auto mb-2" />
          <div className="text-2xl font-bold text-white font-mono">
            {summary?.successful_games ?? 0}
          </div>
          <div className="text-xs text-slate-400">Games Ingested</div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <Layers className="w-5 h-5 text-indigo-400 mx-auto mb-2" />
          <div className="text-2xl font-bold text-white font-mono">
            {summary?.processed_discs ?? 0}
          </div>
          <div className="text-xs text-slate-400">Discs Processed</div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <HardDrive className="w-5 h-5 text-emerald-400 mx-auto mb-2" />
          <div className="text-2xl font-bold text-emerald-400 font-mono">
            {formatBytes(totalOutput)}
          </div>
          <div className="text-xs text-slate-400">Output Written</div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <FolderSync className="w-5 h-5 text-purple-400 mx-auto mb-2" />
          <div className="text-xs font-bold text-slate-200 mt-1">
            {formatBytes(totalSource)} → {formatBytes(totalOutput)}
          </div>
          <div className="text-xs text-slate-400 mt-1.5">Source vs Output</div>
        </div>
      </div>

      {failedIds.length > 0 && (
        <div className="bg-red-950/30 border border-red-900/60 rounded-xl p-4">
          <h3 className="text-sm font-semibold text-red-200">Failed games</h3>
          <ul className="mt-2 space-y-1 text-xs font-mono text-red-100">
            {failedIds.map((id) => (
              <li key={id}>{id}</li>
            ))}
          </ul>
        </div>
      )}

      {partialIds.length > 0 && (
        <div className="bg-amber-950/30 border border-amber-900/60 rounded-xl p-4">
          <h3 className="text-sm font-semibold text-amber-200">Partial multi-disc sets</h3>
          <ul className="mt-2 space-y-1 text-xs font-mono text-amber-100">
            {partialIds.map((id) => (
              <li key={id}>{id}</li>
            ))}
          </ul>
        </div>
      )}

      {recycledBytes !== null && (
        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4">
          <div className="text-xs text-slate-400">Disk space freed</div>
          <div className="text-lg font-mono text-emerald-300">{formatBytes(recycledBytes)}</div>
          <p className="text-[11px] text-slate-500 mt-1">
            Counted from source files moved to the recycle bin.
          </p>
        </div>
      )}

      {/* Safe Trash Cleanup Section */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <div className="flex items-center space-x-2">
              <Trash2 className="w-5 h-5 text-amber-400" />
              <h3 className="text-base font-semibold text-white">
                Safe Source File Cleanup (Recycle Bin)
              </h3>
            </div>
            <p className="text-xs text-slate-400 mt-1 max-w-xl">
              {runFailed
                ? 'Trash stays off while any game failed. Fix the failed discs and run again before moving source dumps.'
                : 'Move the original uncompressed source dumps (.cue, .bin, .gdi) to the Recycle Bin. Files can be restored from there.'}
            </p>
          </div>

          <div>
            {trashedCount !== null ? (
              <div className="px-4 py-2 rounded-xl bg-emerald-950/60 border border-emerald-800/80 text-emerald-300 text-xs font-semibold flex items-center space-x-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                <span>
                  Moved {trashedCount} source files to Trash
                  {recycledBytes !== null ? ` (${formatBytes(recycledBytes)} freed)` : ''}
                </span>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setShowTrashModal(true)}
                disabled={trashLocked}
                className={`px-5 py-2.5 rounded-xl text-xs font-bold flex items-center space-x-2 transition-all shadow-md ${
                  trashLocked
                    ? 'bg-slate-800 text-slate-500 cursor-not-allowed border border-slate-700'
                    : 'bg-amber-500/20 text-amber-300 border border-amber-500/40 hover:bg-amber-500/30'
                }`}
              >
                <Trash2 className="w-4 h-4 text-amber-400" />
                <span>Move Source Dumps to Trash ({filesToTrash.length} files)</span>
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-3">
        <h3 className="text-base font-semibold text-white">Copy verified library</h3>
        <p className="text-xs text-slate-400">
          Copies {outputDir || 'the output folder'} onto a card. The copy is refused when the destination does not have enough free space, and nothing is written in that case.
        </p>
        <div className="flex flex-col sm:flex-row gap-3">
          <input
            type="text"
            aria-label="Copy destination"
            value={deployDest}
            onChange={(event) => setDeployDest(event.target.value)}
            placeholder="e.g. E:/SDCard"
            className="flex-1 px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 font-mono"
          />
          <button
            type="button"
            onClick={() => {
              void deployLibrary().catch(() => undefined);
            }}
            className="px-4 py-2.5 rounded-xl bg-cyan-500/20 text-cyan-200 border border-cyan-500/40 text-xs font-bold"
          >
            Copy verified library
          </button>
        </div>
        {deployError && <p className="text-xs text-red-300">{deployError}</p>}
        {deployedNames.length > 0 && (
          <ul className="text-xs font-mono text-slate-300 space-y-1">
            {deployedNames.map((name) => (
              <li key={name}>{name}</li>
            ))}
          </ul>
        )}
      </div>

      {/* Action Footer */}
      <div className="flex justify-end space-x-4 pt-4 border-t border-slate-800">
        <button
          type="button"
          onClick={reset}
          className="px-6 py-3 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 text-xs font-bold flex items-center space-x-2 transition-all shadow-lg shadow-cyan-500/20 active:scale-[0.98]"
        >
          <RotateCcw className="w-4 h-4" />
          <span>Process Another Batch</span>
        </button>
      </div>

      {/* Safe Trash Confirmation Modal */}
      {showTrashModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm">
          <div className="bg-slate-900 border border-slate-700 rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-6 animate-in fade-in zoom-in-95">
            <div className="flex items-start justify-between">
              <div className="flex items-center space-x-3">
                <div className="w-10 h-10 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-center justify-center text-amber-400">
                  <Trash2 className="w-5 h-5" />
                </div>
                <div>
                  <h4 className="text-base font-bold text-white">
                    Move Source Dumps to Recycle Bin?
                  </h4>
                  <p className="text-xs text-slate-400">
                    Safe non-destructive removal
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setShowTrashModal(false)}
                className="text-slate-500 hover:text-slate-300 p-1 rounded-lg"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="bg-slate-950/70 border border-slate-800 rounded-xl p-4 space-y-2 text-xs text-slate-300">
              <p>
                You are about to move <strong className="text-white font-mono">{filesToTrash.length}</strong> original
                source file(s) to your operating system's Recycle Bin / Trash.
              </p>
              <div className="flex items-start space-x-2 pt-1 text-emerald-400 font-medium">
                <Sparkles className="w-4 h-4 shrink-0 mt-0.5" />
                <span>
                  These files are NOT permanently deleted. You can restore them from your OS Recycle Bin at any time if
                  desired.
                </span>
              </div>
            </div>

            <div className="flex justify-end space-x-3">
              <button
                type="button"
                onClick={() => setShowTrashModal(false)}
                className="px-4 py-2 rounded-xl border border-slate-700 hover:bg-slate-800 text-slate-300 text-xs font-semibold"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleConfirmTrash}
                disabled={isTrashing}
                className="px-5 py-2 rounded-xl bg-amber-500 hover:bg-amber-400 text-slate-950 text-xs font-bold flex items-center space-x-1.5 transition-all"
              >
                {isTrashing ? (
                  <span>Moving to Trash...</span>
                ) : (
                  <>
                    <Trash2 className="w-4 h-4" />
                    <span>Confirm Move to Trash</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
