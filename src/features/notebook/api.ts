import { invoke } from '@/shared/api/tauri';
import type { Cell, CellKind, CellRunResult, Notebook, NotebookSummary } from './types';

interface WireCell {
  id: string;
  cell_type: CellKind;
  source: string;
  execution_count: number | null;
}

interface WireNotebook {
  nbformat: number;
  nbformat_minor: number;
  metadata: Notebook['metadata'];
  cells: WireCell[];
}

function fromWireCell(c: WireCell): Cell {
  return { id: c.id, kind: c.cell_type, source: c.source, executionCount: c.execution_count };
}

function toWireCell(c: Cell): WireCell {
  return { id: c.id, cell_type: c.kind, source: c.source, execution_count: c.executionCount };
}

function fromWire(nb: WireNotebook): Notebook {
  return { ...nb, cells: nb.cells.map(fromWireCell) };
}

function toWire(nb: Notebook): WireNotebook {
  return { ...nb, cells: nb.cells.map(toWireCell) };
}

export const notebookApi = {
  list: (): Promise<NotebookSummary[]> => invoke('notebook_list'),

  read: async (id: string): Promise<Notebook> => {
    const wire = await invoke<WireNotebook>('notebook_read', { id });
    return fromWire(wire);
  },

  write: (id: string, nb: Notebook): Promise<void> =>
    invoke('notebook_write', { id, notebook: toWire(nb) }),

  rename: (id: string, newName: string): Promise<NotebookSummary> =>
    invoke('notebook_rename', { id, newName }),

  runCell: (kind: CellKind, source: string, connectionId: string): Promise<CellRunResult> =>
    invoke('notebook_run_cell', { input: { kind, source, connectionId } }),
};
