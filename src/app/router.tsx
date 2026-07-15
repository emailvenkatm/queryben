import { Navigate, Route, Routes } from 'react-router-dom';
import { AppShell } from '@/widgets/app-shell';
import { ConnectionsPage } from '@/pages/connections';
import { QueryEditorPage } from '@/pages/query-editor';
import { SavedQueriesPage } from '@/pages/saved-queries';
import { QueryHistoryPage } from '@/pages/query-history';
import { NotebookPage } from '@/pages/notebook';
import { SchemaComparePage } from '@/pages/schema-compare';
import { SettingsPage } from '@/pages/settings';
import { OnboardingPage } from '@/pages/onboarding';

// Feature agents replace the stub pages above. The route table stays stable.

export function Router(): React.ReactElement {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<ConnectionsPage />} />
        <Route path="/queries" element={<SavedQueriesPage />} />
        <Route path="/history" element={<QueryHistoryPage />} />
        <Route path="/notebook" element={<NotebookPage />} />
        <Route path="/schema-compare" element={<SchemaComparePage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/onboarding" element={<OnboardingPage />} />
        <Route path="/editor/:connectionId?" element={<QueryEditorPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
