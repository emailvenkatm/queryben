// Hand-rolled invoke() wrappers. CellValue::Int(i64) trips specta's
// BigInt-forbidden rule at boot; hand-rolling sidesteps it. Rust fn names are
// snake_case; Tauri 2 camelCases arg keys automatically.
//
// Azure client + tenant IDs come from Vite env so features don't thread them.

import { invoke } from '@tauri-apps/api/core';
import type {
  Connection,
  CreateConnectionInput,
  QueryOutcome,
  SchemaInfo,
  TableInfo,
  TableMetadata,
  TransactionResult,
  UpdateConnectionInput,
} from '@/shared/types';

const AZURE_CLIENT_ID = import.meta.env.VITE_AZURE_AD_CLIENT_ID ?? '';
const AZURE_TENANT_ID = import.meta.env.VITE_AZURE_AD_TENANT_ID ?? '';

// ---- Azure account types ---------------------------------------------------

export interface AzureAccount {
  id: string;
  username: string | null;
  name: string | null;
  tenantId: string | null;
}

export interface AccountRegistryEntry {
  id: string;
  email: string | null;
  displayName: string | null;
  tenantId: string | null;
}

export interface AzureSubscription {
  id: string;
  displayName: string;
  state: string;
}

export interface AzureSqlServer {
  id: string;
  name: string;
  location: string;
  resourceGroup: string;
  fullyQualifiedDomainName: string;
}

export interface AzureSqlDatabase {
  id: string;
  name: string;
  serverId: string;
  location: string;
}

export interface ConnectAzureSqlInput {
  subscriptionId: string;
  serverId: string;
  databaseId: string;
  name: string;
}

// ---- ADS onboarding types --------------------------------------------------

export interface AdsDetectionSummary {
  version: string | null;
  connectionCount: number;
  msalAccountEmail: string | null;
  snippetCount: number;
  installPath: string;
}

export interface AdsImportSummary {
  connectionsImported: number;
  accountsImported: number;
  snippetsImported: number;
}

// ---- AppError discriminated union ------------------------------------------

export type AppErrorPayload =
  | { kind: 'ConnectionFailed'; message: string }
  | { kind: 'AuthFailed'; message: string }
  | { kind: 'QueryFailed'; message: { message: string; line: number | null; column: number | null } }
  | { kind: 'FirewallBlocked'; message: { ip: string; server: string; connectionId: string | null } }
  | { kind: 'Cancelled' }
  | { kind: 'NotFound'; message: string }
  | { kind: 'NotImplemented'; message: string }
  | { kind: 'RateLimited'; message: { retryAfterSeconds: number | null } }
  | { kind: 'Timeout'; message: string }
  | { kind: 'Internal'; message: string };

// ---- Query plan types ------------------------------------------------------

export interface QueryPlanNode {
  id: string;
  operation: string;
  estimatedRows: number | null;
  actualRows: number | null;
  cost: number | null;
  children: QueryPlanNode[];
}

export interface QueryPlan {
  xml: string;
  nodes: QueryPlanNode[];
}

// ---- Import types ----------------------------------------------------------

export type ImportFormat = 'csv' | 'json' | 'excel';

export interface ImportPreview {
  headers: string[];
  rows: string[][];
  inferredTypes: string[];
  totalRows: number;
}

export interface ImportOptions {
  skipHeaderRow: boolean;
  delimiter?: string;
  batchSize?: number;
}

export interface ColumnMapping {
  sourceColumn: string;
  targetColumn: string;
  skip: boolean;
}

export interface ImportResult {
  rowsInserted: number;
  rowsFailed: number;
  errors: Array<{ row: number; message: string }>;
}

// ---- Narrowing helpers -----------------------------------------------------

export function isFirewallBlocked(
  err: unknown,
): err is Extract<AppErrorPayload, { kind: 'FirewallBlocked' }> {
  if (typeof err !== 'object' || err === null) return false;
  const e = err as { kind?: unknown; message?: unknown };
  if (e.kind !== 'FirewallBlocked') return false;
  const m = e.message as { ip?: unknown; server?: unknown } | undefined;
  return typeof m === 'object' && m !== null && typeof m.ip === 'string' && typeof m.server === 'string';
}

export function isAuthFailed(
  err: unknown,
): err is Extract<AppErrorPayload, { kind: 'AuthFailed' }> {
  if (typeof err !== 'object' || err === null) return false;
  const e = err as { kind?: unknown; message?: unknown };
  return e.kind === 'AuthFailed' && typeof e.message === 'string';
}

export function isTimeout(
  err: unknown,
): err is Extract<AppErrorPayload, { kind: 'Timeout' }> {
  if (typeof err !== 'object' || err === null) return false;
  const e = err as { kind?: unknown; message?: unknown };
  return e.kind === 'Timeout' && typeof e.message === 'string';
}

// ---- IPC commands ----------------------------------------------------------

export const commands = {
  createConnection: (input: CreateConnectionInput): Promise<Connection> =>
    invoke('create_connection', { input }),

  listConnections: (): Promise<Connection[]> => invoke('list_connections'),

  deleteConnection: (id: string): Promise<void> =>
    invoke('delete_connection', { id }),

  updateConnection: (input: UpdateConnectionInput): Promise<Connection> =>
    invoke('update_connection', { input }),

  testConnection: (
    input: CreateConnectionInput,
  ): Promise<{ ok: boolean; message?: string; latencyMs?: number }> =>
    invoke('test_connection', { input }),

  executeQuery: (connectionId: string, sql: string): Promise<QueryOutcome> =>
    invoke('execute_query', { connectionId, sql }),

  cancelQuery: (queryId: string): Promise<void> =>
    invoke('cancel_query', { queryId }),

  getSchema: (connectionId: string): Promise<SchemaInfo> =>
    invoke('get_schema', { connectionId }),

  listTables: (connectionId: string, schema: string): Promise<TableInfo[]> =>
    invoke('list_tables', { connectionId, schema }),

  getTableMetadata: (
    connectionId: string,
    schema: string,
    name: string,
  ): Promise<TableMetadata> =>
    invoke('get_table_metadata', { connectionId, schema, name }),

  executeTransaction: (
    connectionId: string,
    statements: string[],
  ): Promise<TransactionResult> =>
    invoke('execute_transaction', { connectionId, statements }),

  azureSignIn: (): Promise<AzureAccount> =>
    invoke('azure_sign_in', { tenantId: AZURE_TENANT_ID, clientId: AZURE_CLIENT_ID }),

  azureSignOut: (): Promise<void> => invoke('azure_sign_out'),

  azureSignOutAccount: (accountId: string): Promise<void> =>
    invoke('azure_sign_out_account', { accountId }),

  azureCurrentAccount: (): Promise<AzureAccount | null> =>
    invoke('azure_current_account'),

  azureListAccounts: (): Promise<AccountRegistryEntry[]> =>
    invoke('azure_list_accounts'),

  listAzureSubscriptions: (accountId?: string): Promise<AzureSubscription[]> =>
    invoke('list_azure_subscriptions', {
      tenantId: AZURE_TENANT_ID,
      clientId: AZURE_CLIENT_ID,
      accountId: accountId ?? null,
    }),

  listAzureSqlServers: (subscriptionId: string, accountId?: string): Promise<AzureSqlServer[]> =>
    invoke('list_azure_sql_servers', {
      tenantId: AZURE_TENANT_ID,
      clientId: AZURE_CLIENT_ID,
      subscriptionId,
      accountId: accountId ?? null,
    }),

  listAzureSqlDatabases: (serverId: string, accountId?: string): Promise<AzureSqlDatabase[]> =>
    invoke('list_azure_sql_databases', {
      tenantId: AZURE_TENANT_ID,
      clientId: AZURE_CLIENT_ID,
      serverId,
      accountId: accountId ?? null,
    }),

  connectAzureSql: (input: ConnectAzureSqlInput, accountId?: string): Promise<Connection> =>
    invoke('connect_azure_sql', {
      tenantId: AZURE_TENANT_ID,
      clientId: AZURE_CLIENT_ID,
      input,
      accountId: accountId ?? null,
    }),

  addFirewallRule: (
    connectionId: string,
    startIp: string,
    endIp: string,
    ruleName: string,
  ): Promise<void> =>
    invoke('add_firewall_rule', { connectionId, startIp, endIp, ruleName }),

  canAddRuleSilently: (connectionId: string): Promise<boolean> =>
    invoke('can_add_rule_silently', { connectionId }),

  hasCachedAzureToken: (connectionId: string): Promise<boolean> =>
    invoke('has_cached_azure_token', { connectionId }),

  getQueryPlan: (connectionId: string, sql: string): Promise<QueryPlan> =>
    invoke('get_query_plan', { connectionId, sql }),

  importPreview: (path: string, format: ImportFormat): Promise<ImportPreview> =>
    invoke('import_preview', { path, format }),

  importExecute: (
    connectionId: string,
    path: string,
    format: ImportFormat,
    targetSchema: string,
    targetTable: string,
    columnMapping: ColumnMapping[],
    options: ImportOptions,
  ): Promise<ImportResult> =>
    invoke('import_execute', { connectionId, path, format, targetSchema, targetTable, columnMapping, options }),

  readThemeOverrideFile: (): Promise<string | null> =>
    invoke('read_theme_override_file'),

  detectAdsInstallation: (): Promise<AdsDetectionSummary | null> =>
    invoke('detect_ads_installation'),

  importFromAds: (): Promise<AdsImportSummary> => invoke('import_from_ads'),
} as const;

export type Commands = typeof commands;

// Re-exported for feature files that import from this module.
export { formatAppErrorForDisplay } from './errors';
