import { useCallback } from 'react';
import type { CellValue, ColumnMeta, CopyTarget, FormatterRegistry } from '../types';
import { defaultRegistry } from '../formatters/index';
import { loadCopyConfig } from '../api';

async function writeClipboard(text: string): Promise<void> {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.style.position = 'fixed';
  ta.style.opacity = '0';
  document.body.appendChild(ta);
  ta.select();
  try { document.execCommand('copy'); } finally { document.body.removeChild(ta); }
}

interface UseResultsCopyOptions {
  registry?: FormatterRegistry;
}

export function useResultsCopy(options: UseResultsCopyOptions = {}) {
  const registry = options.registry ?? defaultRegistry;

  const copy = useCallback(
    async (formatId: string, columns: ColumnMeta[], rows: CellValue[][], target?: CopyTarget): Promise<void> => {
      const formatter = registry[formatId];
      if (!formatter) {
        console.warn(`[results-copy] unknown format: ${formatId}`);
        return;
      }
      const config = loadCopyConfig();
      const text = formatter.format(columns, rows, target, config);
      await writeClipboard(text);
    },
    [registry],
  );

  const availableFormats = (Object.values(registry) as Array<{ id: string; label: string }>).map((f) => ({ id: f.id, label: f.label }));
  return { copy, availableFormats };
}
