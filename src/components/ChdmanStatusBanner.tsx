import React from 'react';
import {
  CheckCircle2,
  AlertTriangle,
  AlertCircle,
  Download,
  FolderSearch,
  RefreshCw,
  Edit3,
} from 'lucide-react';
import { ChdmanStatus } from '../types/plan';

export interface ChdmanStatusBannerProps {
  status: ChdmanStatus | null;
  isDownloading: boolean;
  downloadProgress: number;
  downloadedBytes?: number;
  totalBytes?: number;
  error: string | null;
  onInstall: () => void;
  onBrowse: () => void;
  onRetry?: () => void;
  onChangePath?: () => void;
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export const ChdmanStatusBanner: React.FC<ChdmanStatusBannerProps> = ({
  status,
  isDownloading,
  downloadProgress,
  downloadedBytes,
  totalBytes,
  error,
  onInstall,
  onBrowse,
  onRetry,
  onChangePath,
}) => {
  // Downloading state
  if (isDownloading) {
    const percent = Math.round(downloadProgress);
    const hasBytes = downloadedBytes !== undefined && totalBytes !== undefined && totalBytes > 0;

    return (
      <div className="bg-blue-950/40 border border-blue-800/60 rounded-2xl p-5 shadow-lg space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="w-8 h-8 rounded-lg bg-blue-900/60 border border-blue-700/50 flex items-center justify-center text-blue-400">
              <Download className="w-4 h-4 animate-bounce" />
            </div>
            <div>
              <h4 className="text-sm font-semibold text-white">
                Downloading & Verifying chdman...
              </h4>
              <p className="text-xs text-blue-300/80">
                Fetching platform binary, verifying SHA-256 hash & extracting to managed directory
              </p>
            </div>
          </div>
          <div className="text-right">
            <span className="text-sm font-bold font-mono text-blue-400">{percent}%</span>
            {hasBytes && (
              <p className="text-[11px] font-mono text-slate-400">
                {formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}
              </p>
            )}
          </div>
        </div>

        {/* Progress bar */}
        <div className="w-full bg-slate-900/80 rounded-full h-2 overflow-hidden border border-slate-700/50">
          <div
            className="bg-gradient-to-r from-cyan-400 to-blue-500 h-full rounded-full transition-all duration-200"
            style={{ width: `${Math.min(100, Math.max(0, percent))}%` }}
          />
        </div>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="bg-red-950/40 border border-red-800/60 rounded-2xl p-5 shadow-lg flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="flex items-start space-x-3">
          <div className="w-8 h-8 rounded-lg bg-red-900/60 border border-red-700/50 flex items-center justify-center text-red-400 shrink-0 mt-0.5">
            <AlertCircle className="w-4 h-4" />
          </div>
          <div>
            <h4 className="text-sm font-semibold text-red-200">chdman Setup Error</h4>
            <p className="text-xs text-red-300/90 mt-0.5">{error}</p>
          </div>
        </div>

        <div className="flex items-center space-x-2 shrink-0 self-start sm:self-center">
          <button
            type="button"
            onClick={onRetry || onInstall}
            className="px-3.5 py-1.5 bg-red-600 hover:bg-red-500 text-white text-xs font-semibold rounded-xl transition-all flex items-center space-x-1.5 shadow-sm"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Retry</span>
          </button>
          <button
            type="button"
            onClick={onBrowse}
            className="px-3.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium rounded-xl border border-slate-700 transition-all flex items-center space-x-1.5"
          >
            <FolderSearch className="w-3.5 h-3.5" />
            <span>Browse Local File...</span>
          </button>
        </div>
      </div>
    );
  }

  // Ready state
  if (status?.ready) {
    const sourceLabels: Record<string, string> = {
      system_path: 'System PATH',
      managed_directory: 'Managed Directory',
      custom_path: 'Custom Path',
      missing: 'Unknown',
    };

    return (
      <div className="bg-emerald-950/30 border border-emerald-800/50 rounded-2xl p-4 sm:p-5 shadow-lg flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="flex items-start space-x-3">
          <div className="w-8 h-8 rounded-lg bg-emerald-900/50 border border-emerald-700/40 flex items-center justify-center text-emerald-400 shrink-0 mt-0.5">
            <CheckCircle2 className="w-4 h-4" />
          </div>
          <div className="space-y-1">
            <div className="flex items-center space-x-2 flex-wrap gap-y-1">
              <h4 className="text-sm font-semibold text-emerald-200">
                Compression Engine Ready
              </h4>
              {status.version && (
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-900/60 text-emerald-300 border border-emerald-700/40 font-semibold">
                  v{status.version}
                </span>
              )}
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800/80 text-slate-300 border border-slate-700">
                {sourceLabels[status.source] || status.source}
              </span>
            </div>
            {status.path && (
              <p className="text-xs font-mono text-slate-400 break-all">{status.path}</p>
            )}
          </div>
        </div>

        <button
          type="button"
          onClick={onChangePath || onBrowse}
          className="px-3 py-1.5 bg-slate-800/90 hover:bg-slate-700 text-slate-300 text-xs font-medium rounded-xl border border-slate-700 transition-colors flex items-center space-x-1.5 shrink-0 self-start sm:self-center"
        >
          <Edit3 className="w-3.5 h-3.5" />
          <span>Change...</span>
        </button>
      </div>
    );
  }

  // Missing state
  return (
    <div className="bg-amber-950/30 border border-amber-800/60 rounded-2xl p-5 shadow-lg flex flex-col md:flex-row md:items-center justify-between gap-4">
      <div className="flex items-start space-x-3">
        <div className="w-8 h-8 rounded-lg bg-amber-900/50 border border-amber-700/50 flex items-center justify-center text-amber-400 shrink-0 mt-0.5">
          <AlertTriangle className="w-4 h-4" />
        </div>
        <div className="space-y-1">
          <div className="flex items-center space-x-2">
            <h4 className="text-sm font-semibold text-amber-200">chdman Required</h4>
            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/30 uppercase tracking-wide">
              Action Required
            </span>
          </div>
          <p className="text-xs text-slate-300 max-w-xl leading-relaxed">
            MAME&apos;s <code className="text-amber-300 bg-slate-900/60 px-1 py-0.5 rounded font-mono">chdman</code> tool
            is required to compress retro disc dumps (.cue/.bin & .gdi) into compact, bit-perfect .chd files.
          </p>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 shrink-0 self-start md:self-center">
        <button
          type="button"
          onClick={onInstall}
          className="px-4 py-2 bg-gradient-to-r from-cyan-500 to-blue-600 hover:brightness-110 text-slate-950 font-bold text-xs rounded-xl shadow-md shadow-cyan-500/20 transition-all flex items-center space-x-1.5 active:scale-[0.98]"
        >
          <Download className="w-3.5 h-3.5" />
          <span>Install chdman (1-Click)</span>
        </button>
        <button
          type="button"
          onClick={onBrowse}
          className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium rounded-xl border border-slate-700 transition-all flex items-center space-x-1.5"
        >
          <FolderSearch className="w-3.5 h-3.5" />
          <span>Browse Local File...</span>
        </button>
      </div>
    </div>
  );
};
