import { useEffect, useState } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import { useApplyTableDdl, useGenerateTableDdl, useLoadTableDesign } from '../api';
import type { DdlStatement, TableDesign } from '../types';

function emptyDesign(schema: string, name: string): TableDesign {
  return {
    schema, name,
    columns: [{ name: 'Id', sqlType: 'int', isNullable: false, isIdentity: true, isComputed: false, computedExpression: null, defaultExpression: null, ordinal: 0 }],
    primaryKey: ['Id'],
    pkName: null,
    indexes: [],
    foreignKeys: [],
  };
}

export interface UseTableDesignerReturn {
  isNew: boolean;
  next: TableDesign | null;
  ddl: DdlStatement[];
  isLoading: boolean;
  loadError: unknown;
  applyError: string | null;
  applyOk: string | null;
  isApplying: boolean;
  isGenerating: boolean;
  setNext: React.Dispatch<React.SetStateAction<TableDesign | null>>;
  togglePk: (col: string) => void;
  apply: () => void;
}

export function useTableDesigner(): UseTableDesignerReturn {
  const { connectionId, schema, name } = useParams<{ connectionId: string; schema: string; name: string }>();
  const [params] = useSearchParams();
  const isNew = params.get('new') === '1';
  const navigate = useNavigate();

  const [current, setCurrent] = useState<TableDesign | null>(null);
  const [next, setNext] = useState<TableDesign | null>(null);
  const [ddl, setDdl] = useState<DdlStatement[]>([]);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applyOk, setApplyOk] = useState<string | null>(null);

  const { data: loaded, isLoading, error: loadError } = useLoadTableDesign(
    isNew ? null : (connectionId ?? null),
    isNew ? null : (schema ?? null),
    isNew ? null : (name ?? null),
  );

  const genMut = useGenerateTableDdl();
  const applyMut = useApplyTableDdl();

  useEffect(() => {
    if (isNew && schema && name && next === null) {
      setCurrent(null);
      setNext(emptyDesign(schema, name));
    } else if (loaded && next === null) {
      setCurrent(loaded);
      setNext(structuredClone(loaded));
    }
  }, [loaded, isNew, schema, name, next]);

  // Regenerate DDL on each edit. Diff runs in Rust — no debounce needed.
  useEffect(() => {
    if (!connectionId || !next) return;
    void genMut.mutateAsync({ connectionId, current, next }).then(setDdl).catch(() => setDdl([]));
    // genMut excluded — identity changes each render, not a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [next, current, connectionId]);

  const togglePk = (col: string) => {
    setNext((prev) => {
      if (!prev) return prev;
      const already = prev.primaryKey.includes(col);
      return { ...prev, primaryKey: already ? prev.primaryKey.filter((c) => c !== col) : [...prev.primaryKey, col] };
    });
  };

  const apply = () => {
    if (!connectionId || ddl.length === 0) return;
    setApplyError(null);
    setApplyOk(null);
    void (async () => {
      try {
        const statements = ddl.map((d) => d.sql).filter((sql) => !sql.trim().startsWith('--'));
        if (statements.length === 0) {
          setApplyError("Can't apply automatically. Copy the SQL and run it by hand.");
          return;
        }
        const result = await applyMut.mutateAsync({ connectionId, statements });
        if (!result.committed) {
          setApplyError(result.errorMessage ?? "Couldn't apply. Nothing was saved.");
          return;
        }
        setApplyOk(`Applied ${result.statementCount} change${result.statementCount === 1 ? '' : 's'} in ${result.durationMs} ms.`);
        setCurrent(structuredClone(next));
        if (isNew) navigate(`/designer/${connectionId}/${schema}/${name}`, { replace: true });
      } catch (err) {
        setApplyError(formatAppErrorForDisplay(err));
      }
    })();
  };

  return { isNew, next, ddl, isLoading, loadError, applyError, applyOk, isApplying: applyMut.isPending, isGenerating: genMut.isPending, setNext, togglePk, apply };
}
