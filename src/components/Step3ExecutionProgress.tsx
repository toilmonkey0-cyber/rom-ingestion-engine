import React, { useEffect, useRef, useState } from 'react';
import {
  Cpu,
  Terminal,
  AlertOctagon,
  CheckCircle,
  Copy,
  Check,
  Disc3,
  Loader2,
} from 'lucide-react';
import { useIngestionStore } from '../store/useIngestionStore';

export const Step3ExecutionProgress: React.FC = () => {
  const { plan, isExecuting, gameProgress, discProgress, activeLogs, error, setStep } =
    useIngestionStore();

  const [copied, setCopied] = useState(false);
  const terminalEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    terminalEndRef.current?.scrollIntoView?.({ behavior: 'smooth' });
  }, [activeLogs]);

  const enabledGames = plan?.games.filter((g) => g.enabled) ?? [];
  const totalDiscs = enabledGames.reduce((acc, g) => acc + g.discs.length, 0);

  // Compute completed games and aggregate progress
  const totalPercentage = enabledGames.length > 0
    ? Math.round(
        enabledGames.reduce((acc, g) => {
          const prog = gameProgress[g.id] ?? 0;
          return acc + prog;
        }, 0) / enabledGames.length
      )
    : 0;

  const handleCopyLogs = () => {
    navigator.clipboard.writeText(activeLogs.join('\n'));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="max-w-6xl mx-auto space-y-6 py-6 px-4">
      {/* Top Banner & Status */}
      <div className="bg-slate-900/80 border border-slate-800 rounded-2xl p-6 shadow-xl relative overflow-hidden">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-center space-x-4">
            <div className="w-12 h-12 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shrink-0 shadow-lg shadow-cyan-500/10">
              {isExecuting ? (
                <Loader2 className="w-6 h-6 animate-spin text-cyan-400" />
              ) : (
                <CheckCircle className="w-6 h-6 text-emerald-400" />
              )}
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <h2 className="text-xl font-bold text-white tracking-tight">
                  {isExecuting ? 'Converting & Verifying Discs...' : 'Execution Finished'}
                </h2>
                <span className="text-[10px] uppercase font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700">
                  Worker Pool Active
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-1">
                chdman compression. Each finished CHD is checked for the MComprHD header.
              </p>
            </div>
          </div>

          <div className="flex items-center space-x-3">
            {!isExecuting && (
              <button
                type="button"
                onClick={() => setStep(4)}
                className="px-5 py-2 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 text-xs font-bold transition-all shadow-md shadow-cyan-500/20"
              >
                View Summary
              </button>
            )}
          </div>
        </div>

        {/* Global Progress Bar */}
        <div className="mt-6 space-y-2">
          <div className="flex justify-between text-xs font-medium">
            <span className="text-slate-300 flex items-center space-x-1.5">
              <Cpu className="w-3.5 h-3.5 text-cyan-400" />
              <span>Overall Ingestion Progress</span>
            </span>
            <span className="text-cyan-400 font-mono font-bold text-sm">
              {totalPercentage}%
            </span>
          </div>

          <div className="w-full bg-slate-950 rounded-full h-3.5 overflow-hidden border border-slate-800 p-0.5">
            <div
              className="bg-gradient-to-r from-cyan-500 via-blue-500 to-indigo-500 h-full rounded-full transition-all duration-300 relative overflow-hidden"
              style={{ width: `${Math.max(totalPercentage, 2)}%` }}
            >
              <div className="absolute inset-0 bg-white/20 w-full animate-pulse" />
            </div>
          </div>

          <div className="flex justify-between text-[11px] text-slate-400 pt-1">
            <span>{enabledGames.length} Games ({totalDiscs} Discs)</span>
            <span>Target: Lossless CHD (LZMA / FLAC / CD-Zlib)</span>
          </div>
        </div>
      </div>

      {error && (
        <div className="bg-red-950/40 border border-red-800/60 rounded-xl p-4 text-red-300 flex items-start space-x-3 text-xs">
          <AlertOctagon className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
          <div className="flex-1">{error}</div>
        </div>
      )}

      {/* Two Column Grid: Active Workers & Real-Time Terminal */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left Column: Active Worker Cards */}
        <div className="lg:col-span-5 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-white flex items-center space-x-2">
              <Disc3 className="w-4 h-4 text-cyan-400" />
              <span>Active Worker Jobs</span>
            </h3>
            <span className="text-xs text-slate-400 font-mono">
              {enabledGames.length} jobs queued
            </span>
          </div>

          <div className="space-y-3 max-h-[420px] overflow-y-auto pr-1">
            {enabledGames.map((game) => {
              const currentProg = gameProgress[game.id] ?? 0;
              const isDone = currentProg >= 100;
              const isActive = currentProg > 0 && currentProg < 100;

              return (
                <div
                  key={game.id}
                  className={`p-3.5 rounded-xl border transition-all ${
                    isActive
                      ? 'bg-cyan-950/20 border-cyan-500/50 shadow-sm shadow-cyan-500/10'
                      : isDone
                      ? 'bg-slate-900/40 border-slate-800/80 opacity-70'
                      : 'bg-slate-900/20 border-slate-800/40 opacity-50'
                  }`}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-xs font-semibold text-slate-200 truncate max-w-[200px]" title={game.canonical_title}>
                      {game.canonical_title}
                    </span>
                    <span className="text-[11px] font-mono font-bold text-cyan-400">
                      {isDone ? 'Done' : `${Math.round(currentProg)}%`}
                    </span>
                  </div>

                  <div className="w-full bg-slate-950 rounded-full h-1.5 overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-all duration-200 ${
                        isDone
                          ? 'bg-emerald-500'
                          : isActive
                          ? 'bg-cyan-400'
                          : 'bg-slate-700'
                      }`}
                      style={{ width: `${currentProg}%` }}
                    />
                  </div>

                  <div className="mt-2 space-y-1">
                    {game.discs.map((disc) => {
                      const discPct = discProgress[`${game.id}:${disc.disc_number}`]
                        ?? (game.discs.length === 1 ? currentProg : 0);
                      return (
                        <div key={disc.disc_number} className="flex justify-between text-[10px] text-slate-400 font-mono">
                          <span>Disc {disc.disc_number}</span>
                          <span>{Math.round(discPct)}%</span>
                        </div>
                      );
                    })}
                  </div>
                  <div className="flex items-center justify-between text-[10px] text-slate-400 mt-2 font-mono">
                    <span className="uppercase">{game.platform}</span>
                    <span>{game.discs.length} disc{game.discs.length > 1 ? 's' : ''}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Right Column: Terminal Console */}
        <div className="lg:col-span-7 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <Terminal className="w-4 h-4 text-emerald-400" />
              <span className="text-sm font-semibold text-white">Live Execution Terminal</span>
            </div>
            <button
              type="button"
              onClick={handleCopyLogs}
              className="text-xs text-slate-400 hover:text-slate-200 flex items-center space-x-1.5 px-2.5 py-1 rounded bg-slate-800 border border-slate-700"
            >
              {copied ? (
                <>
                  <Check className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Copied</span>
                </>
              ) : (
                <>
                  <Copy className="w-3.5 h-3.5" />
                  <span>Copy Logs</span>
                </>
              )}
            </button>
          </div>

          <div className="bg-slate-950 border border-slate-800 rounded-xl p-4 font-mono text-xs text-slate-300 h-[420px] overflow-y-auto flex flex-col justify-between shadow-inner">
            <div className="space-y-1">
              <div className="text-slate-500 pb-2 border-b border-slate-800/80 mb-2">
                // rom-ingest worker log stream: chdman subprocess stdout/stderr
              </div>
              {activeLogs.length === 0 ? (
                <div className="text-slate-600 italic">Initializing runner...</div>
              ) : (
                activeLogs.map((log, index) => {
                  const isError = log.includes('ERROR');
                  const isSuccess = log.includes('status: verified') || log.includes('complete');
                  return (
                    <div
                      key={index}
                      className={`leading-relaxed ${
                        isError
                          ? 'text-red-400 font-semibold'
                          : isSuccess
                          ? 'text-emerald-400'
                          : 'text-slate-300'
                      }`}
                    >
                      {log}
                    </div>
                  );
                })
              )}
              <div ref={terminalEndRef} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
