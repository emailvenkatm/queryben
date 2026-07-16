import { invoke } from '@tauri-apps/api/core';
import type { QueryOutcome, TableMetadata, TransactionResult } from '@/shared/types';

export const queryApi = {
  execute: (connectionId: string, sql: string): Promise<QueryOutcome> =>
    invoke('execute_query', { connectionId, sql }),

  cancel: (queryId: string): Promise<void> =>
    invoke('cancel_query', { queryId }),

  executeTransaction: (connectionId: string, statements: string[]): Promise<TransactionResult> =>
    invoke('execute_transaction', { connectionId, statements }),

  getTableMetadata: (connectionId: string, schema: string, name: string): Promise<TableMetadata> =>
    invoke('get_table_metadata', { connectionId, schema, name }),

  canAddRuleSilently: (connectionId: string): Promise<boolean> =>
    invoke('can_add_rule_silently', { connectionId }),

  addFirewallRule: (connectionId: string, startIp: string, endIp: string, ruleName: string): Promise<void> =>
    invoke('add_firewall_rule', { connectionId, startIp, endIp, ruleName }),
};
