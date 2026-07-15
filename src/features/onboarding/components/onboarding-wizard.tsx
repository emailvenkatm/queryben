import { useCallback, useEffect, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { useNavigate } from 'react-router-dom';
import { commands } from '@/shared/api/tauri-bindings';
import type { AdsDetectionSummary, AdsImportSummary } from '@/shared/api/tauri-bindings';
import { useOnboarding } from '../hooks/use-onboarding';
import { WelcomeStep } from './welcome-step';
import { ImportAdsStep } from './import-ads-step';
import { ConnectChoiceStep } from './connect-choice-step';
import { AzureSignInStep } from './azure-sign-in-step';
import { AllSetStep } from './all-set-step';
import {
  dot,
  footer,
  footerText,
  overlay,
  progressBar,
  skipCorner,
  titlebar,
  titleLabel,
  trafficLight,
  windowShell,
} from './wizard-styles';

type Step = 1 | 2 | 3 | 4 | 5;

const STEP_LABELS: Record<Step, string> = {
  1: 'Step 1 of 5',
  2: 'Step 2 of 5',
  3: 'Step 3 of 5',
  4: 'Step 4 of 5',
  5: 'Setup complete',
};

export function OnboardingWizard() {
  const { isFirstRun, markComplete, skipAll } = useOnboarding();
  const [step, setStep] = useState<Step>(1);
  const [detection, setDetection] = useState<AdsDetectionSummary | null>(null);
  const [detectionLoaded, setDetectionLoaded] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importSummary, setImportSummary] = useState<AdsImportSummary | null>(null);
  const [azurePending, setAzurePending] = useState(false);
  const navigate = useNavigate();

  // Probe ADS on mount so step 2 knows whether to render.
  useEffect(() => {
    let cancelled = false;
    void commands
      .detectAdsInstallation()
      .then((r) => { if (!cancelled) { setDetection(r); setDetectionLoaded(true); } })
      .catch(() => { if (!cancelled) setDetectionLoaded(true); });
    return () => { cancelled = true; };
  }, []);

  const finish = useCallback(() => {
    markComplete({ importedFromAds: importSummary !== null });
    navigate('/editor');
  }, [markComplete, navigate, importSummary]);

  const goNext = useCallback(() => {
    setStep((cur) => {
      if (cur === 1) return (detectionLoaded && detection === null) ? 3 : 2;
      if (cur === 2) return 3;
      if (cur === 3) return 4;
      if (cur === 4) return 5;
      return cur;
    });
  }, [detection, detectionLoaded]);

  const goBack = useCallback(() => {
    setStep((cur) => {
      if (cur === 5) return 4;
      if (cur === 4) return 3;
      if (cur === 3) return (detectionLoaded && detection === null) ? 1 : 2;
      if (cur === 2) return 1;
      return cur;
    });
  }, [detection, detectionLoaded]);

  const runImport = useCallback(async (): Promise<AdsImportSummary | null> => {
    setImporting(true);
    try {
      const s = await commands.importFromAds();
      setImportSummary(s);
      setStep(3);
      return s;
    } catch {
      return null;
    } finally {
      setImporting(false);
    }
  }, []);

  const runAzureSignIn = useCallback(async () => {
    setAzurePending(true);
    try {
      await commands.azureSignIn();
      setStep(5);
    } catch {
      // User cancelled or auth failed — stay on step 4.
    } finally {
      setAzurePending(false);
    }
  }, []);

  const connectionCount = useMemo(() => {
    if (importSummary) return importSummary.connectionsImported;
    return detection?.connectionCount ?? 0;
  }, [detection, importSummary]);

  const signedInEmail = useMemo(() => {
    if (importSummary && importSummary.accountsImported > 0) return detection?.msalAccountEmail ?? null;
    return detection?.msalAccountEmail ?? null;
  }, [detection, importSummary]);

  if (!isFirstRun) return null;

  return (
    <div style={overlay} role="dialog" aria-modal aria-label="QueryBen setup">
      <div style={windowShell}>
        <div style={titlebar}>
          <div style={trafficLight('#FF5F57')} />
          <div style={trafficLight('#FFBD2E')} />
          <div style={trafficLight('#28C840')} />
          <span style={titleLabel}>QueryBen</span>
          <div style={{ width: 52 }} />
        </div>

        <div style={progressBar} role="progressbar" aria-label={`Setup step ${step} of 5`} aria-valuenow={step} aria-valuemax={5}>
          {([1, 2, 3, 4, 5] as Step[]).map((i) => (
            <div key={i} style={dot(i < step ? 'done' : i === step ? 'active' : 'inactive')} />
          ))}
        </div>

        {step >= 2 && step < 5 && (
          <button type="button" onClick={skipAll} style={skipCorner}>Skip</button>
        )}

        {step === 1 && <WelcomeStep onGetStarted={goNext} onSkipAll={skipAll} />}

        {step === 2 && detection !== null && (
          <ImportAdsStep detection={detection} importing={importing} onImport={runImport} onSkip={() => setStep(3)} onBack={goBack} />
        )}
        {step === 2 && detection === null && detectionLoaded && <AdsSkipBridge onContinue={() => setStep(3)} />}

        {step === 3 && (
          <ConnectChoiceStep
            onAzure={() => setStep(4)}
            onSqlAuth={() => { markComplete({ importedFromAds: importSummary !== null }); navigate('/', { replace: true }); }}
            onSkip={() => setStep(5)}
            onBack={goBack}
          />
        )}
        {step === 4 && (
          <AzureSignInStep onOpenBrowser={() => { void runAzureSignIn(); }} onBack={goBack} pending={azurePending} />
        )}
        {step === 5 && (
          <AllSetStep connectionCount={connectionCount} signedInEmail={signedInEmail} onOpenEditor={finish} onBack={goBack} />
        )}

        <div style={footer}>
          <span style={footerText}>v0.1.0-alpha</span>
          <span style={footerText}>{STEP_LABELS[step]}</span>
        </div>
      </div>
    </div>
  );
}

// When the user navigates back into step 2 after ADS was auto-skipped, slide
// them forward immediately. Practically invisible on modern hardware.
function AdsSkipBridge({ onContinue }: { onContinue: () => void }) {
  useEffect(() => { onContinue(); }, [onContinue]);
  const style: CSSProperties = { flex: 1 };
  return <div style={style} />;
}
