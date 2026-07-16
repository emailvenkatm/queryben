import { ArrowLeftIcon, Loader2Icon } from 'lucide-react';
import { cn } from '@/shared/lib/cn';
import { SignInButton, useAzureAuth } from '@/features/azure-auth/index';
import { SubscriptionPicker } from './subscription-picker';
import { SqlServerPicker } from './sql-server-picker';
import { DatabasePicker } from './database-picker';
import { useAzureWizard, type WizardStep } from '../hooks/use-azure-wizard';

interface AzureWizardProps {
  onSuccess: () => void;
}

const STEPS: { id: WizardStep; label: string }[] = [
  { id: 'sign-in', label: 'Sign in' },
  { id: 'subscription', label: 'Subscription' },
  { id: 'server', label: 'Server' },
  { id: 'database', label: 'Database' },
];

const STEP_HEADINGS: Record<WizardStep, string> = {
  'sign-in': 'Sign in with Microsoft',
  subscription: 'Choose a subscription',
  server: 'Choose a SQL server',
  database: 'Choose a database',
};

function ProgressDots({ current }: { current: WizardStep }) {
  return (
    <nav aria-label="Connection wizard progress" className="flex items-center justify-center gap-2 mb-4">
      {STEPS.map((s, i) => {
        const currentIndex = STEPS.findIndex((x) => x.id === current);
        const isPast = i < currentIndex;
        const isActive = s.id === current;
        return (
          <div
            key={s.id}
            aria-current={isActive ? 'step' : undefined}
            title={s.label}
            className={cn(
              'h-2 rounded-full transition-all',
              isActive && 'bg-jade w-4',
              isPast && 'bg-jade/60 w-2',
              !isActive && !isPast && 'bg-border w-2',
            )}
          />
        );
      })}
    </nav>
  );
}

export function AzureWizard({ onSuccess }: AzureWizardProps) {
  const { isAuthenticated } = useAzureAuth();
  const {
    step,
    subscription,
    server,
    activeAccountId,
    setActiveAccountId,
    connecting,
    connectError,
    connectStatus,
    pickSubscription,
    pickServer,
    pickDatabase,
    goBack,
  } = useAzureWizard(isAuthenticated, onSuccess);

  const canGoBack = step !== 'sign-in' && step !== 'subscription';

  return (
    <div className="flex flex-col gap-2">
      <ProgressDots current={step} />

      <div className="flex items-center gap-2 mb-1">
        {canGoBack && (
          <button
            type="button"
            onClick={goBack}
            aria-label="Go back"
            className="h-7 w-7 shrink-0 flex items-center justify-center rounded text-muted-foreground hover:text-foreground"
          >
            <ArrowLeftIcon className="h-4 w-4" aria-hidden="true" />
          </button>
        )}
        <h3 className="text-sm font-semibold text-foreground">{STEP_HEADINGS[step]}</h3>
      </div>

      {step === 'sign-in' && (
        <div className="flex flex-col items-center gap-4 py-10">
          <p className="text-sm text-muted-foreground text-center max-w-xs">
            Sign in with your Azure account. QueryBen will list the SQL databases you can reach.
          </p>
          <SignInButton size="lg" />
        </div>
      )}

      {step === 'subscription' && (
        <SubscriptionPicker
          onPick={pickSubscription}
          activeAccountId={activeAccountId}
          onSelectAccount={setActiveAccountId}
        />
      )}

      {step === 'server' && subscription && (
        <SqlServerPicker
          subscriptionId={subscription.subscriptionId}
          onPick={pickServer}
          accountId={activeAccountId}
        />
      )}

      {step === 'database' && server && (
        <>
          <DatabasePicker
            serverId={server.id}
            onPick={pickDatabase}
            accountId={activeAccountId}
          />
          {connecting && (
            <div className="flex items-center justify-center gap-2 pt-2 text-sm text-muted-foreground" role="status" aria-live="polite">
              <Loader2Icon className="h-4 w-4 animate-spin" aria-hidden="true" />
              <span>{connectStatus ?? 'Connecting…'}</span>
            </div>
          )}
          {connectError && (
            <p className="text-sm text-destructive" role="alert">{connectError}</p>
          )}
        </>
      )}
    </div>
  );
}
