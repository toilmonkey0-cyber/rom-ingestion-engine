import React, { useEffect } from 'react';
import {
  FolderInput,
  FolderOutput,
  KeyRound,
  Sparkles,
  Gamepad2,
  HardDrive,
  Info,
  CheckCircle,
  AlertCircle,
  Image,
} from 'lucide-react';
import { FrontendPreset } from '../types/plan';
import { useIngestionStore } from '../store/useIngestionStore';
import { ChdmanStatusBanner } from './ChdmanStatusBanner';

interface PresetOption {
  id: FrontendPreset;
  title: string;
  badge?: string;
  description: string;
  multidiscFolder: string;
}

/** Matches `get_multidisc_subfolder` in the Rust planner. Every preset writes this directory today. */
const MULTIDISC_SUBFOLDER = '.discs/';

const PRESET_OPTIONS: PresetOption[] = [
  {
    id: 'anbernicstock',
    title: 'Anbernic Stock OS',
    badge: 'Recommended',
    description: 'RG35XX / RG40XX folder names. Multi-disc CHD files are written into .discs, and the M3U sits beside that folder.',
    multidiscFolder: MULTIDISC_SUBFOLDER,
  },
  {
    id: 'onionos',
    title: 'OnionOS / GarlicOS',
    description: 'Miyoo Mini and GarlicOS folder names. Multi-disc CHD files are written into .discs.',
    multidiscFolder: MULTIDISC_SUBFOLDER,
  },
  {
    id: 'esde',
    title: 'ES-DE (EmulationStation)',
    description: 'EmulationStation folder names. Multi-disc CHD files are written into .discs.',
    multidiscFolder: MULTIDISC_SUBFOLDER,
  },
  {
    id: 'batocera',
    title: 'Batocera / Knulli',
    description: 'Batocera and Knulli folder names. Multi-disc CHD files are written into .discs.',
    multidiscFolder: MULTIDISC_SUBFOLDER,
  },
];

export const Step1Config: React.FC = () => {
  const {
    inputDir,
    outputDir,
    datPath,
    regionPriority,
    preset,
    apiKey,
    isScanning,
    error,
    chdmanStatus,
    isDownloadingChdman,
    chdmanDownloadProgress,
    chdmanDownloadedBytes,
    chdmanTotalBytes,
    chdmanError,
    mediaOptions,
    setInputDir,
    setOutputDir,
    setDatPath,
    setRegionPriority,
    setPreset,
    setApiKey,
    setMediaOptions,
    startScan,
    checkChdmanStatus,
    downloadChdman,
    setCustomChdmanPath,
  } = useIngestionStore();

  useEffect(() => {
    checkChdmanStatus();
  }, [checkChdmanStatus]);

  const handleBrowseChdman = () => {
    const defaultVal = chdmanStatus?.path || '';
    const chosen = window.prompt(
      'Enter absolute path to chdman binary (e.g. C:/Tools/chdman.exe):',
      defaultVal
    );
    if (chosen && chosen.trim()) {
      setCustomChdmanPath(chosen.trim());
    }
  };

  const handleFillDemo = () => {
    setInputDir('D:/Roms/Unsorted_Dumps');
    setOutputDir('E:/SDCard/Roms');
  };

  return (
    <div className="max-w-4xl mx-auto space-y-8 py-6 px-4">
      {/* Introduction Card */}
      <div className="bg-gradient-to-r from-slate-900 to-slate-800/80 border border-slate-800 rounded-2xl p-6 shadow-xl relative overflow-hidden">
        <div className="absolute right-0 top-0 bottom-0 w-1/3 bg-gradient-to-l from-cyan-500/10 to-transparent pointer-events-none" />
        <div className="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div className="inline-flex items-center space-x-2 text-cyan-400 text-xs font-semibold uppercase tracking-wider mb-2">
              <Sparkles className="w-3.5 h-3.5" />
              <span>Non-Destructive Processing</span>
            </div>
            <h2 className="text-2xl font-bold text-white tracking-tight">
              Ingest & Modernize Your Retro Disc Dumps
            </h2>
            <p className="text-sm text-slate-300 mt-1 max-w-xl">
              Convert bulky <code className="text-cyan-300 bg-slate-800/60 px-1 py-0.5 rounded">.BIN/.CUE</code> and{' '}
              <code className="text-cyan-300 bg-slate-800/60 px-1 py-0.5 rounded">.GDI</code> dumps into compressed,
              bit-perfect <code className="text-cyan-300 bg-slate-800/60 px-1 py-0.5 rounded">.CHD</code> disc images
              with automated multi-disc playlist generation.
            </p>
          </div>
          <div className="flex flex-wrap gap-1.5 self-start">
            {['PSX', 'Saturn', 'Dreamcast', 'Sega CD', 'PCE-CD'].map((plat) => (
              <span
                key={plat}
                className="text-[11px] font-mono px-2.5 py-1 rounded bg-slate-800/80 border border-slate-700 text-slate-300"
              >
                {plat}
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* chdman Engine Readiness Banner */}
      <ChdmanStatusBanner
        status={chdmanStatus}
        isDownloading={isDownloadingChdman}
        downloadProgress={chdmanDownloadProgress}
        downloadedBytes={chdmanDownloadedBytes}
        totalBytes={chdmanTotalBytes}
        error={chdmanError}
        onInstall={downloadChdman}
        onBrowse={handleBrowseChdman}
        onRetry={downloadChdman}
        onChangePath={handleBrowseChdman}
      />

      {error && (
        <div className="bg-red-950/40 border border-red-800/60 rounded-xl p-4 text-red-300 flex items-start space-x-3 text-sm">
          <AlertCircle className="w-5 h-5 text-red-400 shrink-0 mt-0.5" />
          <div className="flex-1">{error}</div>
        </div>
      )}

      {/* Directory Configuration */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-6">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-white flex items-center space-x-2">
            <HardDrive className="w-4 h-4 text-cyan-400" />
            <span>Folder Selection</span>
          </h3>
          <button
            type="button"
            onClick={handleFillDemo}
            className="text-xs text-cyan-400 hover:text-cyan-300 transition-colors underline"
          >
            Fill Sample Paths
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Source Directory */}
          <div className="space-y-2">
            <label className="text-xs font-semibold text-slate-300 flex items-center space-x-2">
              <FolderInput className="w-4 h-4 text-slate-400" />
              <span>Source Dump Folder (Read-Only)</span>
            </label>
            <input
              type="text"
              value={inputDir}
              onChange={(e) => setInputDir(e.target.value)}
              placeholder="e.g. D:/Emulation/Dumps/PlayStation"
              className="w-full px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500 transition-all font-mono"
            />
            <p className="text-[11px] text-slate-400">
              Scans recursively for .cue, .gdi, .iso, and .bin track files. Original files are never modified in-place.
            </p>
          </div>

          {/* Destination Directory */}
          <div className="space-y-2">
            <label className="text-xs font-semibold text-slate-300 flex items-center space-x-2">
              <FolderOutput className="w-4 h-4 text-slate-400" />
              <span>Target Ingestion Directory</span>
            </label>
            <input
              type="text"
              value={outputDir}
              onChange={(e) => setOutputDir(e.target.value)}
              placeholder="e.g. E:/Roms/PSX or D:/Organized"
              className="w-full px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500 transition-all font-mono"
            />
            <p className="text-[11px] text-slate-400">
              Where compressed .chd images and .m3u playlists will be organized according to the selected preset.
            </p>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div className="space-y-2">
            <label className="text-xs font-semibold text-slate-300" htmlFor="dat-path">
              Redump DAT (optional)
            </label>
            <input
              id="dat-path"
              type="text"
              value={datPath}
              onChange={(e) => setDatPath(e.target.value)}
              placeholder="e.g. D:/dats/psx.dat"
              aria-label="Redump DAT"
              className="w-full px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500 transition-all font-mono"
            />
            <p className="text-[11px] text-slate-400">
              A user DAT names releases by checksum or serial. Leave blank to keep filename fallback, which starts disabled.
            </p>
          </div>
          <div className="space-y-2">
            <label className="text-xs font-semibold text-slate-300" htmlFor="region-priority">
              Region priority
            </label>
            <input
              id="region-priority"
              type="text"
              value={regionPriority}
              onChange={(e) => setRegionPriority(e.target.value)}
              placeholder="USA, Europe, Japan"
              aria-label="Region priority"
              className="w-full px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500 transition-all font-mono"
            />
            <p className="text-[11px] text-slate-400">
              Comma-separated. The first matching region stays enabled. Other regions of the same edition become alternates.
            </p>
          </div>
        </div>
      </div>

      {/* Preset Selector */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-white flex items-center space-x-2">
            <Gamepad2 className="w-4 h-4 text-cyan-400" />
            <span>Target Handheld / Frontend Preset</span>
          </h3>
          <span className="text-xs text-slate-400">Determines folder naming & multi-disc hiding</span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {PRESET_OPTIONS.map((opt) => {
            const isSelected = preset === opt.id;
            return (
              <div
                key={opt.id}
                role="button"
                tabIndex={0}
                onClick={() => setPreset(opt.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') setPreset(opt.id);
                }}
                className={`text-left p-4 rounded-xl border transition-all cursor-pointer relative flex flex-col justify-between ${
                  isSelected
                    ? 'bg-cyan-950/30 border-cyan-500/80 shadow-md shadow-cyan-500/10 ring-1 ring-cyan-500/40'
                    : 'bg-slate-950/40 border-slate-800 hover:border-slate-700 hover:bg-slate-900/50'
                }`}
              >
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="font-semibold text-sm text-white">{opt.title}</span>
                    {opt.badge && (
                      <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-cyan-500 text-slate-950 uppercase tracking-wide">
                        {opt.badge}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-slate-400 leading-relaxed mb-3">{opt.description}</p>
                </div>
                <div className="flex items-center justify-between pt-2 border-t border-slate-800/60 text-[11px] text-slate-400 font-mono">
                  <span>Subfolder: {opt.multidiscFolder}</span>
                  {isSelected && <CheckCircle className="w-4 h-4 text-cyan-400" />}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Media & Artwork Options */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-white flex items-center space-x-2">
            <Image className="w-4 h-4 text-purple-400" />
            <span>Box Art & Media Downloads</span>
          </h3>
          <span className="text-xs text-slate-400">Libretro Open Thumbnails CDN</span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <label className="flex items-center space-x-3 p-3 rounded-xl bg-slate-950/40 border border-slate-800 cursor-pointer hover:border-slate-700 transition-colors">
            <input
              type="checkbox"
              checked={mediaOptions.download_boxart}
              onChange={() => setMediaOptions({ download_boxart: !mediaOptions.download_boxart })}
              className="w-4 h-4 rounded bg-slate-900 border-slate-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-slate-900 cursor-pointer"
            />
            <div>
              <div className="text-xs font-semibold text-slate-200">Front Box Art</div>
              <div className="text-[10px] text-slate-400">Cover art thumbnails</div>
            </div>
          </label>

          <label className="flex items-center space-x-3 p-3 rounded-xl bg-slate-950/40 border border-slate-800 cursor-pointer hover:border-slate-700 transition-colors">
            <input
              type="checkbox"
              checked={mediaOptions.download_screenshots}
              onChange={() => setMediaOptions({ download_screenshots: !mediaOptions.download_screenshots })}
              className="w-4 h-4 rounded bg-slate-900 border-slate-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-slate-900 cursor-pointer"
            />
            <div>
              <div className="text-xs font-semibold text-slate-200">Gameplay Screenshots</div>
              <div className="text-[10px] text-slate-400">In-game screen captures</div>
            </div>
          </label>

          <label className="flex items-center space-x-3 p-3 rounded-xl bg-slate-950/40 border border-slate-800 cursor-pointer hover:border-slate-700 transition-colors">
            <input
              type="checkbox"
              checked={mediaOptions.download_titles}
              onChange={() => setMediaOptions({ download_titles: !mediaOptions.download_titles })}
              className="w-4 h-4 rounded bg-slate-900 border-slate-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-slate-900 cursor-pointer"
            />
            <div>
              <div className="text-xs font-semibold text-slate-200">Title Screens</div>
              <div className="text-[10px] text-slate-400">Game title screen captures</div>
            </div>
          </label>
        </div>

        <div className="flex items-start space-x-2 text-xs text-slate-400">
          <Info className="w-3.5 h-3.5 mt-0.5 text-slate-500 shrink-0" />
          <span>
            Downloads free community-maintained artwork from the Libretro Thumbnails CDN. Images are saved to preset-appropriate
            media folders on your target device (e.g. <code className="text-purple-300 bg-slate-800/60 px-1 rounded">Imgs/</code> for Anbernic).
          </span>
        </div>
      </div>

      {/* Advanced / Optional AI Key */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-white flex items-center space-x-2">
            <KeyRound className="w-4 h-4 text-amber-400" />
            <span>TypeSafe Jev AI Key (Optional)</span>
          </h3>
          <span className="text-xs text-amber-400/80 bg-amber-950/40 border border-amber-900/60 px-2 py-0.5 rounded-full">
            Enhanced Matching
          </span>
        </div>

        <div className="space-y-2">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="jev_live_..."
            className="w-full px-3.5 py-2.5 bg-slate-950/70 border border-slate-700/80 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-amber-500/40 focus:border-amber-500 transition-all font-mono"
          />
          <div className="flex items-start space-x-2 text-xs text-slate-400">
            <Info className="w-3.5 h-3.5 mt-0.5 text-slate-500 shrink-0" />
            <span>
              If Redump hash matching misses (e.g. for romhacks, undubs, or custom rips), the built-in Jev AI client
              fuzzy-matches titles, detects disc sequences, and generates clean canonical metadata automatically.
            </span>
          </div>
        </div>
      </div>

      {/* Action Button */}
      <div className="pt-2 flex justify-end">
        <button
          type="button"
          onClick={startScan}
          disabled={isScanning || !inputDir.trim() || !outputDir.trim()}
          className={`px-8 py-3.5 rounded-xl font-semibold text-sm flex items-center space-x-2 transition-all shadow-lg ${
            isScanning || !inputDir.trim() || !outputDir.trim()
              ? 'bg-slate-800 text-slate-500 cursor-not-allowed border border-slate-700'
              : 'bg-gradient-to-r from-cyan-500 to-blue-600 text-slate-950 font-bold hover:brightness-110 shadow-cyan-500/20 active:scale-[0.98]'
          }`}
        >
          {isScanning ? (
            <>
              <div className="w-4 h-4 border-2 border-slate-950 border-t-transparent rounded-full animate-spin" />
              <span>Scanning Dumps & Building Plan...</span>
            </>
          ) : (
            <>
              <Sparkles className="w-4 h-4" />
              <span>Scan & Generate Ingestion Plan</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
};
