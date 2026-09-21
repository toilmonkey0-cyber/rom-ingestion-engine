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
  Image as ImageIcon,
  Palette,
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
  const summary = useIngestionStore((s) => s.summary);
  const isTrashing = useIngestionStore((s) => s.isTrashing);
  const trashedCount = useIngestionStore((s) => s.trashedCount);
  const trashSourceFiles = useIngestionStore((s) => s.trashSourceFiles);
  const reset = useIngestionStore((s) => s.reset);
  const error = useIngestionStore((s) => s.error);
  const isFinishing = useIngestionStore((s) => s.isFinishing);
  const finishProgress = useIngestionStore((s) => s.finishProgress);
  const finishResult = useIngestionStore((s) => s.finishResult);
  const finishLibrary = useIngestionStore((s) => s.finishLibrary);

  const [showTrashModal, setShowTrashModal] = useState(false);
  const [trashError, setTrashError] = useState<string | null>(null);

  const totalSource = summary?.total_source_bytes ?? 0;
  const totalOutput = summary?.total_output_bytes ?? 0;
  const savedBytes = totalSource > totalOutput ? totalSource - totalOutput : 0;
  const savingsPct =
    totalSource > 0 ? Math.round((savedBytes / totalSource) * 100) : 0;

  const failedGames = summary?.failed_games ?? 0;
  const successfulGames = summary?.successful_games ?? 0;
  const allFailed = summary !== null && successfulGames === 0;
  const partialFailure = summary !== null && failedGames > 0 && successfulGames > 0;

  const filesToTrash = summary?.source_files_to_trash ?? [];
  const hasFilesToTrash = filesToTrash.length > 0 && trashedCount === null;

  const handleConfirmTrash = async () => {
    setTrashError(null);
    try {
      await trashSourceFiles();
      setShowTrashModal(false);
    } catch (err: unknown) {
      // Keep the modal open and show the error inside it — the page-level
      // banner is hidden behind the modal overlay.
      const msg = err instanceof Error ? err.message : String(err);
      setTrashError(msg);
    }
  };

  const heroIcon = allFailed || partialFailure ? (
    <AlertTriangle className="w-8 h-8 text-amber-400" />
  ) : (
    <ShieldCheck className="w-8 h-8 text-emerald-400" />
  );

  return (
    <div className="max-w-4xl mx-auto space-y-8 py-6 px-4">
      {/* Hero Result Card */}
      <div
        className={`bg-gradient-to-r ${
          allFailed
            ? 'from-red-950/40 via-slate-900 to-slate-900 border-red-800/50'
            : partialFailure
            ? 'from-amber-950/40 via-slate-900 to-slate-900 border-amber-800/50'
            : 'from-emerald-950/40 via-slate-900 to-slate-900 border-emerald-800/50'
        } border rounded-2xl p-6 shadow-2xl relative overflow-hidden`}
      >
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-6">
          <div className="flex items-center space-x-4">
            <div
              className={`w-14 h-14 rounded-2xl border flex items-center justify-center shrink-0 shadow-lg ${
                allFailed
                  ? 'bg-red-500/10 border-red-500/30 text-red-400 shadow-red-500/10'
                  : partialFailure
                  ? 'bg-amber-500/10 border-amber-500/30 text-amber-400 shadow-amber-500/10'
                  : 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400 shadow-emerald-500/10'
              }`}
            >
              {heroIcon}
            </div>
            <div>
              {allFailed ? (
                <>
                  <div className="inline-flex items-center space-x-1.5 text-red-400 text-xs font-bold uppercase tracking-wider mb-1">
                    <AlertTriangle className="w-3.5 h-3.5" />
                    <span>No Games Ingested</span>
                  </div>
                  <h2 className="text-2xl font-bold text-white tracking-tight">
                    Ingestion Failed
                  </h2>
                  <p className="text-xs text-slate-300 mt-1 max-w-lg">
                    All {failedGames} planned game(s) failed to convert. Your original source files were
                    not modified. Check the execution log on the previous screen for chdman error details.
                  </p>
                </>
              ) : partialFailure ? (
                <>
                  <div className="inline-flex items-center space-x-1.5 text-amber-400 text-xs font-bold uppercase tracking-wider mb-1">
                    <AlertTriangle className="w-3.5 h-3.5" />
                    <span>Completed with Failures</span>
                  </div>
                  <h2 className="text-2xl font-bold text-white tracking-tight">
                    {successfulGames} Succeeded, {failedGames} Failed
                  </h2>
                  <p className="text-xs text-slate-300 mt-1 max-w-lg">
                    Some games could not be converted and their source files were left untouched.
                    Only fully verified games are eligible for source cleanup below.
                  </p>
                </>
              ) : (
                <>
                  <div className="inline-flex items-center space-x-1.5 text-emerald-400 text-xs font-bold uppercase tracking-wider mb-1">
                    <CheckCircle2 className="w-3.5 h-3.5" />
                    <span>Verified & Validated</span>
                  </div>
                  <h2 className="text-2xl font-bold text-white tracking-tight">
                    Ingestion Completed Successfully!
                  </h2>
                  <p className="text-xs text-slate-300 mt-1 max-w-lg">
                    All planned disc images have been compressed to bit-perfect lossless CHD format and M3U playlists
                    have been generated for multi-disc titles.
                  </p>
                </>
              )}
            </div>
          </div>

          <div
            className={`border rounded-xl px-5 py-3 text-center self-start sm:self-auto shrink-0 ${
              allFailed || partialFailure
                ? 'bg-slate-950/60 border-slate-700/80'
                : 'bg-emerald-950/60 border-emerald-800/80'
            }`}
          >
            <div
              className={`text-2xl font-black font-mono ${
                allFailed || partialFailure ? 'text-slate-300' : 'text-emerald-400'
              }`}
            >
              {savingsPct}%
            </div>
            <div className="text-[11px] text-slate-400 font-medium">Space Saved</div>
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
      <div className={`grid grid-cols-2 sm:grid-cols-4 gap-4 ${failedGames > 0 ? 'sm:grid-cols-5' : ''}`}>
        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <FileCheck className="w-5 h-5 text-cyan-400 mx-auto mb-2" />
          <div className="text-2xl font-bold text-white font-mono">
            {successfulGames}
          </div>
          <div className="text-xs text-slate-400">Games Ingested</div>
        </div>

        {failedGames > 0 && (
          <div className="bg-slate-900/60 border border-red-900/60 rounded-xl p-4 text-center">
            <AlertTriangle className="w-5 h-5 text-red-400 mx-auto mb-2" />
            <div className="text-2xl font-bold text-red-400 font-mono">{failedGames}</div>
            <div className="text-xs text-slate-400">Games Failed</div>
          </div>
        )}

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
            {formatBytes(savedBytes)}
          </div>
          <div className="text-xs text-slate-400">Disk Space Freed</div>
        </div>

        <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-4 text-center">
          <FolderSync className="w-5 h-5 text-purple-400 mx-auto mb-2" />
          <div className="text-xs font-bold text-slate-200 mt-1">
            {formatBytes(totalSource)} → {formatBytes(totalOutput)}
          </div>
          <div className="text-xs text-slate-400 mt-1.5">Source vs Output</div>
        </div>
      </div>

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
              Now that your CHDs have been verified, you can move the original uncompressed source dumps (.cue, .bin,
              .gdi) to your operating system's Recycle Bin / Trash. Original files can be restored if ever needed.
            </p>
          </div>

          <div>
            {trashedCount !== null ? (
              <div className="px-4 py-2 rounded-xl bg-emerald-950/60 border border-emerald-800/80 text-emerald-300 text-xs font-semibold flex items-center space-x-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                <span>Moved {trashedCount} source files to Trash</span>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setShowTrashModal(true)}
                disabled={!hasFilesToTrash || isTrashing}
                className={`px-5 py-2.5 rounded-xl text-xs font-bold flex items-center space-x-2 transition-all shadow-md ${
                  !hasFilesToTrash || isTrashing
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

      {/* Finish Line: Box Art & Playlist Metadata */}
      {successfulGames > 0 && (
        <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-4">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <div className="flex items-center space-x-2">
                <Palette className="w-5 h-5 text-purple-400" />
                <h3 className="text-base font-semibold text-white">
                  Finish Line: Box Art &amp; Playlist Metadata
                </h3>
              </div>
              <p className="text-xs text-slate-400 mt-1 max-w-xl">
                Downloads box art from the libretro thumbnail service (no account needed) and writes{' '}
                <code className="text-purple-300 bg-slate-800/60 px-1 py-0.5 rounded">gamelist.xml</code> metadata
                for ES-DE / Batocera. Other frontends get artwork placed next to each playlist for auto-loading.
                Your card looks finished the moment it boots.
              </p>
            </div>

            <div className="shrink-0">
              {finishResult ? (
                <div className="px-4 py-2 rounded-xl bg-purple-950/60 border border-purple-800/80 text-purple-300 text-xs font-semibold flex items-center space-x-2">
                  <CheckCircle2 className="w-4 h-4 text-purple-400" />
                  <span>
                    {finishResult.artwork_downloaded} art downloaded · {finishResult.gamelists_written} gamelist
                    {finishResult.gamelists_written === 1 ? '' : 's'} written
                    {finishResult.artwork_failed > 0
                      ? ` · ${finishResult.artwork_failed} not found (skipped)`
                      : ''}
                  </span>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => finishLibrary(true)}
                  disabled={isFinishing}
                  className={`px-5 py-2.5 rounded-xl text-xs font-bold flex items-center space-x-2 transition-all shadow-md ${
                    isFinishing
                      ? 'bg-slate-800 text-slate-500 cursor-not-allowed border border-slate-700'
                      : 'bg-purple-500/20 text-purple-300 border border-purple-500/40 hover:bg-purple-500/30'
                  }`}
                >
                  {isFinishing ? (
                    <>
                      <div className="w-4 h-4 border-2 border-purple-400 border-t-transparent rounded-full animate-spin" />
                      <span>
                        {finishProgress
                          ? `Fetching artwork ${finishProgress.completed}/${finishProgress.total}…`
                          : 'Preparing…'}
                      </span>
                    </>
                  ) : (
                    <>
                      <ImageIcon className="w-4 h-4 text-purple-400" />
                      <span>Download Box Art &amp; Generate Metadata</span>
                    </>
                  )}
                </button>
              )}
            </div>
          </div>

          {isFinishing && finishProgress && (
            <div className="space-y-1.5">
              <div className="w-full bg-slate-950 rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-gradient-to-r from-purple-500 to-fuchsia-500 h-full rounded-full transition-all duration-300"
                  style={{
                    width: `${finishProgress.total > 0 ? (finishProgress.completed / finishProgress.total) * 100 : 0}%`,
                  }}
                />
              </div>
              <p className="text-[11px] text-slate-500 font-mono truncate">
                {finishProgress.title}…
              </p>
            </div>
          )}
        </div>
      )}

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

            {trashError && (
              <div className="bg-red-950/60 border border-red-800/60 rounded-xl p-3 text-red-300 flex items-start space-x-2 text-xs">
                <AlertTriangle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
                <div className="flex-1">
                  <p className="font-semibold mb-0.5">Nothing was moved to the trash.</p>
                  <p className="text-red-200/80">{trashError}</p>
                </div>
              </div>
            )}

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
