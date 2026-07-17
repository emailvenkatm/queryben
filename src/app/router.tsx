import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { AppShell } from '@/widgets/app-shell';
import { Palette } from '@/features/command-palette';
import { ObjectTree } from '@/features/object-explorer';
import { SidebarAccountAvatar } from '@/features/azure-auth';
import { hasSeenOnboarding } from '@/features/onboarding';
import { ConnectionsPage } from '@/pages/connections';
import { QueryEditorPage } from '@/pages/query-editor';
import { SavedQueriesPage } from '@/pages/saved-queries';
import { QueryHistoryPage } from '@/pages/query-history';
import { NotebookPage } from '@/pages/notebook';
import { SchemaComparePage } from '@/pages/schema-compare';
import { SettingsPage } from '@/pages/settings';
import { OnboardingPage } from '@/pages/onboarding';

function OnboardingGuard({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  if (!hasSeenOnboarding() && location.pathname !== '/onboarding') {
    return <Navigate to="/onboarding" replace />;
  }
  return <>{children}</>;
}

export function Router(): React.ReactElement {
  return (
    <Routes>
      <Route
        element={
          <OnboardingGuard>
            <AppShell
              commandPalette={(props) => <Palette {...props} />}
              objectTree={<ObjectTree />}
              accountAvatar={<SidebarAccountAvatar />}
            />
          </OnboardingGuard>
        }
      >
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
