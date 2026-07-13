import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import type { CellKind, Notebook } from '../types';
import {
  emptyNotebook,
  newCell,
  nextUntitledName,
  useNotebook,
  useNotebookList,
  useRenameNotebook,
  useSaveNotebook,
} from './use-notebook';

export function useNotebookScreen() {
  const { id: routeId } = useParams();
  const navigate = useNavigate();
  const selectedId = routeId ?? null;

  const { data: loaded } = useNotebook(selectedId);
  const { data: summaries } = useNotebookList();
  const saveMutation = useSaveNotebook();
  const renameMutation = useRenameNotebook();

  const [draft, setDraft] = useState<Notebook | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saveToast, setSaveToast] = useState<string | null>(null);

  useEffect(() => {
    if (loaded) {
      setDraft(loaded);
      setDirty(false);
    } else if (!selectedId) {
      setDraft(null);
    }
  }, [loaded, selectedId]);

  function create(activeConnectionId: string | null) {
    const names = (summaries ?? []).map((s) => s.name);
    const name = nextUntitledName(names);
    const nb = emptyNotebook(name);
    if (activeConnectionId) nb.metadata.connectionId = activeConnectionId;
    setDraft(nb);
    setDirty(true);
    navigate(`/notebooks/${encodeURIComponent(name)}`);
  }

  async function save() {
    if (!draft || !selectedId) return;
    await saveMutation.mutateAsync({ id: selectedId, notebook: draft });
    setDirty(false);
    setSaveToast('Saved');
    setTimeout(() => setSaveToast(null), 2000);
  }

  async function rename(newName: string) {
    const trimmed = newName.trim();
    if (!trimmed || !selectedId || !draft || trimmed === selectedId) return;
    if (dirty) {
      await saveMutation.mutateAsync({ id: selectedId, notebook: draft });
      setDirty(false);
    }
    const summary = await renameMutation.mutateAsync({ id: selectedId, newName: trimmed });
    navigate(`/notebooks/${encodeURIComponent(summary.id)}`, { replace: true });
  }

  function updateSource(idx: number, source: string) {
    setDraft((prev) => {
      if (!prev) return prev;
      const cells = prev.cells.slice();
      const cell = cells[idx];
      if (!cell) return prev;
      cells[idx] = { ...cell, source };
      return { ...prev, cells };
    });
    setDirty(true);
  }

  function deleteCell(idx: number) {
    setDraft((prev) => {
      if (!prev) return prev;
      const cells = prev.cells.slice();
      cells.splice(idx, 1);
      return { ...prev, cells: cells.length > 0 ? cells : [newCell('sql')] };
    });
    setDirty(true);
  }

  function insertBelow(idx: number, kind: CellKind) {
    setDraft((prev) => {
      if (!prev) return prev;
      const cells = prev.cells.slice();
      cells.splice(idx + 1, 0, newCell(kind));
      return { ...prev, cells };
    });
    setDirty(true);
  }

  function setConnection(connId: string | null) {
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, metadata: { ...prev.metadata, connectionId: connId } };
    });
    setDirty(true);
  }

  function select(id: string) {
    navigate(`/notebooks/${encodeURIComponent(id)}`);
  }

  return {
    selectedId,
    draft,
    dirty,
    saveToast,
    isSaving: saveMutation.isPending,
    isRenaming: renameMutation.isPending,
    summaries: summaries ?? [],
    create,
    save,
    rename,
    updateSource,
    deleteCell,
    insertBelow,
    setConnection,
    select,
  };
}
