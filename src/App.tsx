import React, { useEffect } from 'react';
import { WizardHeader } from './components/WizardHeader';
import { Step1Config } from './components/Step1Config';
import { Step2DryRunTable } from './components/Step2DryRunTable';
import { Step3ExecutionProgress } from './components/Step3ExecutionProgress';
import { Step4Summary } from './components/Step4Summary';
import { useIngestionStore } from './store/useIngestionStore';

export const App: React.FC = () => {
  const step = useIngestionStore((s) => s.step);
  const hydrateFromSettings = useIngestionStore((s) => s.hydrateFromSettings);

  useEffect(() => {
    hydrateFromSettings();
  }, [hydrateFromSettings]);

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col selection:bg-cyan-500 selection:text-white relative">
      {/* Background Decorative Glow */}
      <div className="fixed top-0 left-1/4 w-96 h-96 bg-cyan-500/5 rounded-full blur-3xl pointer-events-none -z-10" />
      <div className="fixed bottom-0 right-1/4 w-96 h-96 bg-blue-600/5 rounded-full blur-3xl pointer-events-none -z-10" />

      {/* Persistent Step Navigation Header */}
      <WizardHeader currentStep={step} />

      {/* Main Wizard Step Content */}
      <main className="flex-1 flex flex-col justify-start pb-12">
        {step === 1 && <Step1Config />}
        {step === 2 && <Step2DryRunTable />}
        {step === 3 && <Step3ExecutionProgress />}
        {step === 4 && <Step4Summary />}
      </main>
    </div>
  );
};

export default App;
