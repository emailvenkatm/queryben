import { useState } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
  type ColumnDef,
  type Table,
} from '@tanstack/react-table';
import type { CellValue, QueryResult } from '@/shared/types';

function buildColumns(result: QueryResult): ColumnDef<CellValue[]>[] {
  return result.columns.map((col, idx) => {
    const id = col.name || `col_${idx}`;
    const header = col.name || `(No column name)`;
    return { id, accessorFn: (row: CellValue[]) => row[idx], header, size: 160 };
  });
}

interface UseResultsTableReturn {
  table: Table<CellValue[]>;
  selectedRows: Set<number>;
  toggleRow: (idx: number, multi: boolean) => void;
}

export function useResultsTable(result: QueryResult): UseResultsTableReturn {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [selectedRows, setSelectedRows] = useState<Set<number>>(new Set());

  const table = useReactTable({
    data: result.rows,
    columns: buildColumns(result),
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const toggleRow = (idx: number, multi: boolean): void => {
    setSelectedRows((prev) => {
      if (multi) {
        const next = new Set(prev);
        if (next.has(idx)) next.delete(idx);
        else next.add(idx);
        return next;
      }
      return new Set([idx]);
    });
  };

  return { table, selectedRows, toggleRow };
}
