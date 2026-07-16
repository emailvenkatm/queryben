// Re-export from shared bindings for consumers that prefer the feature path.
export type {
  AzureAccount,
  AccountRegistryEntry,
  AzureSubscription,
  AzureSqlServer,
  AzureSqlDatabase,
} from '@/shared/api/tauri-bindings';

// Feature-only types not in tauri-bindings.

export type SubscriptionState =
  | 'Enabled'
  | 'Warned'
  | 'PastDue'
  | 'Disabled'
  | 'Deleted'
  | (string & {});

export type DatabaseStatus =
  | 'Online'
  | 'Restoring'
  | 'RecoveryPending'
  | 'Recovering'
  | 'Recovery'
  | 'Paused'
  | 'Suspect'
  | 'Offline'
  | (string & {});

export interface ConnectAzureSqlInput {
  displayName: string;
  serverFqdn: string;
  database: string;
  serverId: string;
  nickname?: string | null;
  color?: 'cream' | 'amber' | 'jade' | 'rose' | 'violet' | 'graphite' | null;
}
