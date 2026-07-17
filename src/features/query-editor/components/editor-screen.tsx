import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useSearchParams } from 'react-router-dom';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { usePendingChangesStore } from '@/shared/stores/pending-changes';
import { commands, isAuthFailed, isFirewallBlocked } from '@/shared/api/tauri-bindings';
import {
  FirewallDialog,
  FirewallToast,
  loadPreferSubnet,
  generateRuleName,
  toSubnetRange,
  type FirewallBlockedPayload,
} from '@/features/firewall';
import { AiPanel } from '@/features/ai-assist';
import { QueryPlanView, useQueryPlan } from '@/features/query-plan';
import { SaveQueryDialog } from '@/features/saved-queries';
import { SnippetPalette } from '@/features/snippets';
import { useQueryExecution, useTabResult, resultKeys, primaryResult } from '../hooks/use-query-execution';
import { TabStrip } from './tab-strip';
import { EditorToolbar } from './editor-toolbar';
import { MonacoEditor } from './monaco-editor';
import { ResultsGrid } from './results-grid';
import { BrowseGrid } from './browse-grid';
import { PendingChangesTray } from './pending-changes-tray';

export function EditorScreen() {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTabId = searchParams.get('tab');

  const tabs = useOpenTabsStore((s) => s.tabs);
  const activeTab = tabs.find((t) => t.id === activeTabId) ?? null;
  const { updateTabSql, setActiveTab, closeTab, openTab } = useOpenTabsStore();
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);

  const { execute, cancel, isPending, pendingTabId, error: executeError } = useQueryExecution();
  const isActiveTabLoading = isPending && pendingTabId === activeTabId;
  const { data: outcome } = useTabResult(activeTabId ?? '');
  const result = primaryResult(outcome);
  const clearPendingForTab = usePendingChangesStore((s) => s.clearForTab);
  const qc = useQueryClient();

  const [commitToast, setCommitToast] = useState<string | null>(null);
  const [firewallToast, setFirewallToast] = useState<string | null>(null);
  const [firewallDismissed, setFirewallDismissed] = useState(false);
  const [autoSilentActive, setAutoSilentActive] = useState(false);
  const autoFireKeyRef = useRef<string | null>(null);

  const rawFirewall = useMemo<FirewallBlockedPayload | null>(() => {
    if (!executeError || !isFirewallBlocked(executeError)) return null;
    return executeError.message as unknown as FirewallBlockedPayload;
  }, [executeError]);

  const firewallPayload = useMemo<FirewallBlockedPayload | null>(() => {
    if (firewallDismissed || autoSilentActive) return null;
    return rawFirewall;
  }, [rawFirewall, firewallDismissed, autoSilentActive]);

  useEffect(() => { setFirewallDismissed(false); }, [executeError]);

  // Auto-silent firewall fix: probe → add rule → retry without showing the dialog.
  useEffect(() => {
    if (!rawFirewall) { autoFireKeyRef.current = null; return; }
    if (!rawFirewall.connectionId) return;
    const key = `${rawFirewall.connectionId}:${rawFirewall.ip}`;
    if (autoFireKeyRef.current === key) return;
    autoFireKeyRef.current = key;

    let cancelled = false;
    void (async () => {
      const { connectionId, ip } = rawFirewall;
      if (!connectionId) return;
      let canSilent = false;
      try { canSilent = await commands.canAddRuleSilently(connectionId); } catch { canSilent = false; }
      if (cancelled || !canSilent) return;

      setAutoSilentActive(true);
      setCommitToast(null);
      const preferSubnet = loadPreferSubnet();
      const range = preferSubnet ? toSubnetRange(ip) : { start: ip, end: ip };
      const ruleName = generateRuleName(preferSubnet);
      setFirewallToast(preferSubnet ? `Added ${range.start}–${range.end} to Azure firewall, retrying…` : `Added ${ip} to Azure firewall, retrying…`);

      try {
        await commands.addFirewallRule(connectionId, range.start, range.end, ruleName);
        if (cancelled) return;
        if (activeConnectionId && activeTabId && activeTab) {
          void execute({ connectionId: activeConnectionId, sql: activeTab.sql, tabId: activeTabId });
        }
      } catch {
        if (cancelled) return;
        setFirewallToast(null);
        setAutoSilentActive(false);
        autoFireKeyRef.current = null;
      }
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rawFirewall]);

  useEffect(() => {
    if (!firewallToast) return;
    const t = setTimeout(() => { setFirewallToast(null); setAutoSilentActive(false); }, 2000);
    return () => clearTimeout(t);
  }, [firewallToast]);

  // Auto-run browse tabs on first open.
  useEffect(() => {
    if (!activeTab || !activeTabId || !activeConnectionId) return;
    if (!activeTab.browseTable || result || isPending) return;
    void execute({ connectionId: activeConnectionId, sql: activeTab.sql, tabId: activeTabId });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab?.id, activeConnectionId, activeTab?.browseTable]);

  const refetchBrowse = useCallback(() => {
    if (!activeConnectionId || !activeTabId || !activeTab) return;
    qc.removeQueries({ queryKey: resultKeys.byTab(activeTabId) });
    void execute({ connectionId: activeConnectionId, sql: activeTab.sql, tabId: activeTabId });
  }, [activeConnectionId, activeTabId, activeTab, execute, qc]);

  const onCommitted = useCallback((committedCount: number) => {
    refetchBrowse();
    const label = committedCount === 1 ? '1 change committed' : `${committedCount} changes committed`;
    setFirewallToast(null);
    setCommitToast(label);
    setTimeout(() => setCommitToast(null), 2000);
  }, [refetchBrowse]);

  const handleTabChange = (tabId: string) => { setActiveTab(tabId); setSearchParams({ tab: tabId }); };

  const handleTabClose = (tabId: string) => {
    clearPendingForTab(tabId);
    closeTab(tabId);
    const nextTab = tabs.find((t) => t.id !== tabId);
    if (nextTab) setSearchParams({ tab: nextTab.id });
    else setSearchParams({});
  };

  const handleNewTab = () => {
    const connectionId = activeTab?.connectionId ?? activeConnectionId;
    if (!connectionId) return;
    const focusedId = openTab({ id: crypto.randomUUID(), connectionId, title: 'New query', sql: '', isDirty: false, createdAt: new Date().toISOString() });
    setSearchParams({ tab: focusedId });
  };

  const [planOpen, setPlanOpen] = useState(false);
  const { plan, isLoading: planLoading, error: planError, fetchPlan, reset: resetPlan } = useQueryPlan();

  const handleExplain = useCallback(() => {
    if (!activeConnectionId || !activeTab) return;
    setPlanOpen(true);
    fetchPlan({ connectionId: activeConnectionId, sql: activeTab.sql });
  }, [activeConnectionId, activeTab, fetchPlan]);

  const [aiOpen, setAiOpen] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);
  const [snippetsOpen, setSnippetsOpen] = useState(false);

  const handleInsertSql = useCallback((sql: string) => {
    if (!activeTabId) return;
    const next = activeTab?.sql ? `${activeTab.sql.trimEnd()}\n\n${sql}\n` : `${sql}\n`;
    updateTabSql(activeTabId, next);
  }, [activeTabId, activeTab?.sql, updateTabSql]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.shiftKey || !(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== 'p') return;
      e.preventDefault();
      setSnippetsOpen((v) => !v);
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);

  if (!activeTab) {
    return (
      <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center', background: 'var(--color-bg)' }}>
        <p style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>
          Select a connection to open a query tab.
        </p>
      </div>
    );
  }

  const handleRun = () => {
    if (!activeConnectionId || !activeTabId) return;
    void execute({ connectionId: activeConnectionId, sql: activeTab.sql, tabId: activeTabId });
  };

  return (
    <div style={{ display: 'flex', height: '100%', flexDirection: 'row', overflow: 'hidden' }}>
      <div style={{ display: 'flex', flex: 1, flexDirection: 'column', overflow: 'hidden', background: 'var(--color-bg)' }}>
        <TabStrip tabs={tabs} activeTabId={activeTabId} onTabChange={handleTabChange} onTabClose={handleTabClose} onNewTab={handleNewTab} />

        {!activeTab.browseTable && (
          <>
            <EditorToolbar
              isPending={isPending}
              hasConnection={Boolean(activeConnectionId)}
              onRun={handleRun}
              onCancel={() => { /* cancel not yet wired: activeQueryId is null */ }}
              onExplain={handleExplain}
              onToggleAi={() => setAiOpen((v) => !v)}
              aiOpen={aiOpen}
              onSaveQuery={() => setSaveOpen(true)}
              onOpenSnippets={() => setSnippetsOpen(true)}
            />
            <div style={{ height: 300, minHeight: 120, borderBottom: '1px solid rgba(26,46,42,0.10)', flexShrink: 0 }}>
              <MonacoEditor value={activeTab.sql} onChange={(sql) => updateTabSql(activeTab.id, sql)} onRun={handleRun} />
            </div>
          </>
        )}

        <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column', position: 'relative' }}>
          {executeError && !isFirewallBlocked(executeError) && (
            <div
              role="alert"
              style={{ flexShrink: 0, padding: '8px 14px', background: isAuthFailed(executeError) ? 'rgba(42,87,81,0.08)' : 'rgba(220,38,38,0.06)', borderBottom: `1px solid ${isAuthFailed(executeError) ? 'rgba(42,87,81,0.25)' : 'rgba(192,57,43,0.25)'}`, color: isAuthFailed(executeError) ? 'var(--color-primary-hover)' : 'var(--color-error)', fontSize: 12, fontFamily: 'Geist Mono, monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}
            >
              <strong style={{ fontFamily: 'Geist, sans-serif', fontWeight: 600, marginRight: 6 }}>
                {isAuthFailed(executeError) ? 'Sign-in required:' : 'Query failed:'}
              </strong>
              {isAuthFailed(executeError)
                ? 'Sign in to Azure again from the Connections list to keep going.'
                : executeError instanceof Error
                  ? executeError.message
                  : String((executeError as { message?: unknown })?.message ?? executeError)}
            </div>
          )}

          {commitToast && (
            <div role="status" aria-live="polite" style={{ position: 'absolute', top: 12, right: 16, zIndex: 100, background: 'var(--color-primary-hover)', color: '#fff', padding: '7px 14px', borderRadius: 7, fontSize: 12, fontFamily: 'Geist, sans-serif', fontWeight: 500, boxShadow: '0 2px 10px rgba(26,46,42,0.2)', display: 'flex', alignItems: 'center', gap: 7, animation: 'qb-toast-in 160ms ease' }}>
              <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
                <circle cx="6.5" cy="6.5" r="6" stroke="rgba(255,255,255,0.5)" strokeWidth="1" />
                <path d="M3.5 6.5l2 2 4-4" stroke="#fff" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              {commitToast}
            </div>
          )}

          {activeTab.browseTable && activeConnectionId ? (
            <BrowseGrid
              result={result ?? null}
              connectionId={activeConnectionId}
              tabId={activeTab.id}
              browseTable={activeTab.browseTable}
              isLoading={isActiveTabLoading}
              onRefresh={refetchBrowse}
            />
          ) : (
            <>
              {isPending && (
                <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
                  <span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>Running…</span>
                </div>
              )}
              {!isPending && outcome && <ResultsGrid outcome={outcome} sql={activeTab.sql} browseTable={activeTab.browseTable} />}
              {!isPending && !outcome && (
                <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
                  <span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>Press F5 or Run to execute the query.</span>
                </div>
              )}
            </>
          )}
        </div>

        {activeTab.browseTable && activeConnectionId && (
          <PendingChangesTray tabId={activeTab.id} connectionId={activeConnectionId} onCommitted={onCommitted} />
        )}

        <FirewallToast message={firewallToast} />

        <FirewallDialog
          payload={firewallPayload}
          onClose={() => setFirewallDismissed(true)}
          onRetry={() => {
            if (!activeConnectionId || !activeTabId || !activeTab) return;
            return execute({ connectionId: activeConnectionId, sql: activeTab.sql, tabId: activeTabId });
          }}
        />

        {planOpen && (
          <QueryPlanView
            plan={plan}
            isLoading={planLoading}
            error={planError}
            onClose={() => { setPlanOpen(false); resetPlan(); }}
          />
        )}
      </div>

      <AiPanel connectionId={activeConnectionId ?? null} open={aiOpen} onClose={() => setAiOpen(false)} onInsertSql={handleInsertSql} />

      <SaveQueryDialog open={saveOpen} sql={activeTab.sql} connectionId={activeConnectionId ?? null} onClose={() => setSaveOpen(false)} />

      <SnippetPalette open={snippetsOpen} onClose={() => setSnippetsOpen(false)} onInsert={handleInsertSql} />
    </div>
  );
}
