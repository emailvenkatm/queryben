import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { commands } from '@/shared/api/tauri-bindings';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import { connectionKeys } from '../api';
import { useAzureAccount, useAzureAccounts, type AzureSubscription, type AzureSqlServer, type AzureSqlDatabase } from '@/features/azure-auth/api';
import type { ConnectionColor } from '@/shared/types';

export type WizardStep = 'sign-in' | 'subscription' | 'server' | 'database';

type ConnectProgress =
  | { stage: 'resuming'; attempt: number; maxAttempts: number; waitedMs: number }
  | { stage: 'adding-firewall-rule'; ip: string };

const CONNECT_PROGRESS_EVENT = 'queryben://connect-progress';

export interface UseAzureWizardReturn {
  step: WizardStep;
  subscription: AzureSubscription | null;
  server: AzureSqlServer | null;
  activeAccountId: string | null;
  setActiveAccountId: (id: string | null) => void;
  connecting: boolean;
  connectError: string | null;
  connectStatus: string | null;
  pickSubscription: (sub: AzureSubscription) => void;
  pickServer: (server: AzureSqlServer) => void;
  pickDatabase: (db: AzureSqlDatabase, labels?: { nickname: string | null; color: ConnectionColor | null }) => Promise<void>;
  goBack: () => void;
}

export function useAzureWizard(isAuthenticated: boolean, onSuccess: () => void): UseAzureWizardReturn {
  const [subscription, setSubscription] = useState<AzureSubscription | null>(null);
  const [server, setServer] = useState<AzureSqlServer | null>(null);
  const [activeAccountId, setActiveAccountIdRaw] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [connectStatus, setConnectStatus] = useState<string | null>(null);

  const currentAccountQuery = useAzureAccount();
  const accountsQuery = useAzureAccounts();

  useEffect(() => {
    if (activeAccountId !== null) return;
    const current = currentAccountQuery.data;
    if (current?.homeAccountId) { setActiveAccountIdRaw(current.homeAccountId); return; }
    const first = accountsQuery.data?.[0];
    if (first) setActiveAccountIdRaw(first.accountId);
  }, [activeAccountId, currentAccountQuery.data, accountsQuery.data]);

  const setActiveAccountId = (id: string | null) => {
    setActiveAccountIdRaw(id);
    setSubscription(null);
    setServer(null);
  };

  const qc = useQueryClient();
  const setActiveConnection = useActiveConnectionStore((s) => s.setActiveConnection);
  const openTab = useOpenTabsStore((s) => s.openTab);
  const navigate = useNavigate();

  const step: WizardStep = !isAuthenticated ? 'sign-in'
    : !subscription ? 'subscription'
    : !server ? 'server'
    : 'database';

  const pickSubscription = (sub: AzureSubscription) => {
    setSubscription(sub);
    setServer(null);
  };

  const pickServer = (s: AzureSqlServer) => {
    setServer(s);
  };

  const pickDatabase = async (
    db: AzureSqlDatabase,
    labels?: { nickname: string | null; color: ConnectionColor | null },
  ): Promise<void> => {
    if (!server) return;
    setConnecting(true);
    setConnectError(null);
    setConnectStatus(db.status === 'Paused' ? 'Resuming database — this can take up to a minute' : null);

    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<ConnectProgress>(CONNECT_PROGRESS_EVENT, (event) => {
        const payload = event.payload;
        if (payload.stage === 'resuming') {
          const waitedSec = Math.round(payload.waitedMs / 1000);
          setConnectStatus(`Resuming database… attempt ${payload.attempt} of ${payload.maxAttempts} (waited ${waitedSec}s)`);
        } else if (payload.stage === 'adding-firewall-rule') {
          setConnectStatus(`Adding firewall rule for ${payload.ip}…`);
        }
      });

      const connection = await commands.connectAzureSql(
        {
          serverFqdn: server.fullyQualifiedDomainName,
          database: db.name,
          displayName: `${server.name} / ${db.name}`,
          serverId: server.id,
          nickname: labels?.nickname ?? null,
          color: labels?.color ?? null,
        },
        activeAccountId ?? undefined,
      );

      await qc.invalidateQueries({ queryKey: connectionKeys.list() });
      setActiveConnection(connection.id);

      const effectiveTabId = openTab({
        id: crypto.randomUUID(),
        connectionId: connection.id,
        title: `${connection.database} · ${connection.server}`,
        sql: '',
        isDirty: false,
        createdAt: new Date().toISOString(),
      });

      onSuccess();
      navigate(`/editor?tab=${effectiveTabId}`);
    } catch (err) {
      setConnectError((err as Error).message ?? 'Connection failed');
    } finally {
      if (unlisten) unlisten();
      setConnecting(false);
      setConnectStatus(null);
    }
  };

  const goBack = () => {
    if (step === 'database') { setServer(null); return; }
    if (step === 'server') { setSubscription(null); return; }
  };

  return { step, subscription, server, activeAccountId, setActiveAccountId, connecting, connectError, connectStatus, pickSubscription, pickServer, pickDatabase, goBack };
}
