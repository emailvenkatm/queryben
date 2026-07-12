export function formatAppErrorForDisplay(err: unknown): string {
  if (err === null || err === undefined) return 'Something went wrong.';
  if (typeof err === 'string') return err;
  if (err instanceof Error) return err.message;
  if (typeof err !== 'object') return 'Something went wrong.';

  const e = err as { kind?: unknown; message?: unknown };
  const strMsg = typeof e.message === 'string' ? e.message : undefined;

  switch (e.kind) {
    case 'NotImplemented':
      return strMsg ? `Not ready yet — ${strMsg}` : 'Not ready yet.';
    case 'AuthFailed':
      return strMsg ?? 'Authentication failed.';
    case 'ConnectionFailed':
      return strMsg ?? "Couldn't reach the database.";
    case 'Timeout':
      return strMsg ?? 'Connection timed out.';
    case 'NotFound':
      return strMsg ?? 'Item not found.';
    case 'Cancelled':
      return 'Cancelled.';
    case 'QueryFailed': {
      const q = e.message as { message?: unknown } | undefined;
      if (q && typeof q.message === 'string') return q.message;
      return 'Query failed.';
    }
    case 'FirewallBlocked': {
      const f = e.message as { ip?: unknown; server?: unknown } | undefined;
      if (f && typeof f.ip === 'string' && typeof f.server === 'string') {
        return `Firewall is blocking ${f.ip} from reaching ${f.server}.`;
      }
      return 'A firewall rule is blocking this connection.';
    }
    case 'RateLimited':
      return 'Rate limited. Try again shortly.';
    case 'Internal':
      return strMsg ?? 'Internal error.';
    default:
      return strMsg ?? 'Something went wrong.';
  }
}
