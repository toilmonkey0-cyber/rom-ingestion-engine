import React from 'react';
import { Disc3, FolderCog, TableProperties, Cpu, CheckCircle2 } from 'lucide-react';

interface WizardHeaderProps {
  currentStep: 1 | 2 | 3 | 4;
}

const steps = [
  { id: 1, name: 'Setup & Preset', icon: FolderCog },
  { id: 2, name: 'Plan Review', icon: TableProperties },
  { id: 3, name: 'Compression', icon: Cpu },
  { id: 4, name: 'Complete', icon: CheckCircle2 },
];

export const WizardHeader: React.FC<WizardHeaderProps> = ({ currentStep }) => {
  return (
    <header className="border-b border-slate-800 bg-slate-900/80 backdrop-blur sticky top-0 z-20 px-6 py-4">
      <div className="max-w-7xl mx-auto flex flex-col md:flex-row md:items-center md:justify-between gap-4">
        {/* Brand */}
        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 flex items-center justify-center shadow-lg shadow-cyan-500/20 text-white">
            <Disc3 className="w-6 h-6 animate-spin-slow" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <h1 className="font-bold text-lg tracking-tight text-white">
                ROM Ingestion Engine
              </h1>
              <span className="text-[10px] uppercase font-semibold px-2 py-0.5 rounded-full bg-cyan-950 text-cyan-400 border border-cyan-800/60">
                v1.0 CHD
              </span>
            </div>
            <p className="text-xs text-slate-400">
              Lossless CHD compression & multi-disc M3U playlist generator
            </p>
          </div>
        </div>

        {/* Stepper Navigation */}
        <nav aria-label="Progress" className="flex items-center space-x-1 sm:space-x-3">
          {steps.map((s, idx) => {
            const Icon = s.icon;
            const isCurrent = currentStep === s.id;
            const isCompleted = currentStep > s.id;

            return (
              <React.Fragment key={s.id}>
                <div
                  className={`flex items-center space-x-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                    isCurrent
                      ? 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 shadow-sm shadow-cyan-500/10'
                      : isCompleted
                      ? 'text-emerald-400 bg-emerald-950/20 border border-emerald-900/40'
                      : 'text-slate-500 border border-transparent'
                  }`}
                >
                  <span
                    className={`w-5 h-5 rounded-full flex items-center justify-center text-[11px] font-bold ${
                      isCurrent
                        ? 'bg-cyan-500 text-slate-950'
                        : isCompleted
                        ? 'bg-emerald-500 text-slate-950'
                        : 'bg-slate-800 text-slate-400'
                    }`}
                  >
                    {isCompleted ? '✓' : s.id}
                  </span>
                  <span className="hidden lg:inline-flex items-center space-x-1">
                    <Icon className="w-3 h-3" />
                    <span>{s.name}</span>
                  </span>
                </div>

                {idx < steps.length - 1 && (
                  <div
                    className={`h-0.5 w-3 sm:w-6 transition-colors ${
                      currentStep > s.id ? 'bg-emerald-500/50' : 'bg-slate-800'
                    }`}
                  />
                )}
              </React.Fragment>
            );
          })}
        </nav>
      </div>
    </header>
  );
};
