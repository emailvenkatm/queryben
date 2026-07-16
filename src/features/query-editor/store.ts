// Tab workspace state lives here. Thin re-export from shared so the public
// API stays clean — consumers import from @/features/query-editor, not from
// the shared store directly.
export { useOpenTabsStore } from '@/shared/stores/open-tabs';
export { usePendingChangesStore } from '@/shared/stores/pending-changes';
export type { PendingChange, PendingChangeKind } from '@/shared/stores/pending-changes';
