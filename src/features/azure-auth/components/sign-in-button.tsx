import { useAzureAuth } from '../api';

interface SignInButtonProps {
  size?: 'sm' | 'default' | 'lg';
}

function MsLogo() {
  return (
    <svg width="16" height="16" viewBox="0 0 21 21" aria-hidden="true" focusable="false">
      <rect x="1" y="1" width="9" height="9" fill="#f25022" />
      <rect x="11" y="1" width="9" height="9" fill="#7fba00" />
      <rect x="1" y="11" width="9" height="9" fill="#00a4ef" />
      <rect x="11" y="11" width="9" height="9" fill="#ffb900" />
    </svg>
  );
}

const PADDING: Record<NonNullable<SignInButtonProps['size']>, string> = {
  sm: '5px 12px',
  default: '7px 16px',
  lg: '9px 20px',
};

export function SignInButton({ size = 'default' }: SignInButtonProps) {
  const { signIn, isAuthenticated, signingIn, signInError } = useAzureAuth();

  if (isAuthenticated) return null;

  const handleClick = async (): Promise<void> => {
    try {
      await signIn();
    } catch {
      // Error surfaced via signInError below.
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
      <button
        type="button"
        onClick={() => { void handleClick(); }}
        disabled={signingIn}
        aria-label="Sign in with Microsoft"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: PADDING[size],
          background: 'var(--color-accent)',
          color: '#fff',
          border: 'none',
          borderRadius: 8,
          fontSize: 13,
          fontWeight: 500,
          fontFamily: 'Geist, sans-serif',
          cursor: signingIn ? 'wait' : 'pointer',
          opacity: signingIn ? 0.7 : 1,
        }}
      >
        {signingIn ? (
          <span style={{ width: 16, height: 16, borderRadius: '50%', border: '2px solid rgba(255,255,255,0.3)', borderTopColor: '#fff', animation: 'qb-spin 0.8s linear infinite', display: 'inline-block' }} aria-hidden="true" />
        ) : (
          <MsLogo />
        )}
        {signingIn ? 'Waiting for browser…' : 'Sign in with Microsoft'}
      </button>
      {signInError ? (
        <p role="alert" style={{ fontSize: 12, color: 'var(--color-error)', textAlign: 'center', maxWidth: 280 }}>
          {(signInError as Error).message ?? 'Sign-in failed'}
        </p>
      ) : null}
    </div>
  );
}
