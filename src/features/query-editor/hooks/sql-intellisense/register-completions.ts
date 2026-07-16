// Monaco completion provider for the `sql` language. Wired in from
// MonacoEditor's onMount, which hands us the Monaco namespace so we don't
// have to add `monaco-editor` as a direct dep (it's a peer of
// @monaco-editor/react and would double-bundle if we imported it directly).
//
// The provider is idempotent: registering twice would surface every entry
// twice in the suggestion popup, so we guard with a module-level Set keyed
// by "language:instanceId" — that survives Vite HMR reloads (module state
// persists across hot updates until a full page refresh).

import type { Monaco } from '@monaco-editor/react';
import {
  TSQL_KEYWORDS,
  TSQL_FUNCTIONS,
  TSQL_TYPES,
  type StaticCompletion,
} from './completions';
import type { SqlSchemaSnapshot } from './use-schema-snapshot';

// Sort-order tiers. Lower number = higher in the popup. Live schema wins
// over static catalog entries because the user's own table/column names
// almost always outrank the T-SQL vocabulary in relevance.
const SORT_COLUMN = '1';
const SORT_TABLE = '2';
const SORT_FUNCTION = '3';
const SORT_KEYWORD = '4';
const SORT_TYPE = '5';

// Registered providers, keyed by the Monaco instance identity. Prevents
// double-registration under HMR — Monaco doesn't dedupe providers itself.
const registered = new WeakSet<Monaco>();

// A cheap snapshot ref that the provider closes over. The React side
// updates this on every render via updateSnapshot(); the provider reads it
// at suggestion time. We can't close over React state inside the provider
// callback because the provider is registered once at mount.
let currentSnapshot: SqlSchemaSnapshot = { tables: [], allColumns: [] };

export function updateSchemaSnapshot(snapshot: SqlSchemaSnapshot): void {
  currentSnapshot = snapshot;
}

export function registerSqlCompletions(monaco: Monaco): void {
  if (registered.has(monaco)) return;
  registered.add(monaco);

  // Minimal shapes for the two callback args — avoids taking a direct
  // dep on monaco-editor just for the ITextModel / Position types.
  interface ProviderModel {
    getLineContent(line: number): string;
    getWordUntilPosition(pos: ProviderPosition): {
      startColumn: number;
      endColumn: number;
      word: string;
    };
  }
  interface ProviderPosition {
    lineNumber: number;
    column: number;
  }

  monaco.languages.registerCompletionItemProvider('sql', {
    triggerCharacters: ['.', ' ', '\n'],
    provideCompletionItems(model: ProviderModel, position: ProviderPosition) {
      const line = model.getLineContent(position.lineNumber);
      const upToCursor = line.slice(0, position.column - 1);

      // Word under the cursor — Monaco uses this to filter suggestions
      // client-side once we return them. Setting `range` correctly is what
      // lets Monaco replace the partial word instead of appending to it.
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      const context = classifyContext(upToCursor);
      const suggestions: unknown[] = [];

      const K = monaco.languages.CompletionItemKind;

      // Column-of-table lookup — user typed `Employees.` and we can show
      // that table's specific column list. Falls back silently when we
      // don't have column metadata cached for the qualifier.
      if (context.kind === 'column-of') {
        const cols = columnsFor(context.qualifier);
        for (const c of cols) {
          suggestions.push({
            label: c.name,
            kind: K.Field,
            detail: c.type || 'column',
            insertText: c.name,
            sortText: SORT_COLUMN + c.name,
            range,
          });
        }
        return { suggestions };
      }

      // FROM / JOIN / UPDATE / INTO — offer table names only. Skips
      // keywords + columns to keep the list focused, matching ADS.
      if (context.kind === 'table-slot') {
        pushTables(suggestions, K, range);
        return { suggestions };
      }

      // Default: everything, ranked so live schema surfaces above statics.
      pushColumns(suggestions, K, range);
      pushTables(suggestions, K, range);
      pushStatic(suggestions, TSQL_FUNCTIONS, K.Function, SORT_FUNCTION, range);
      pushStatic(suggestions, TSQL_KEYWORDS, K.Keyword, SORT_KEYWORD, range);
      pushStatic(suggestions, TSQL_TYPES, K.TypeParameter, SORT_TYPE, range);

      return { suggestions };
    },
  });
}

// --- context classifier ------------------------------------------------------

type CompletionContext =
  | { kind: 'column-of'; qualifier: string }
  | { kind: 'table-slot' }
  | { kind: 'default' };

// Cheap regex-based sniff — no SQL parser here. Good enough for the three
// contexts that materially change what should appear. Anything more nuanced
// (alias resolution, CTE scope, subquery walls) is out of scope for v1.
function classifyContext(upToCursor: string): CompletionContext {
  // `<identifier>.` right before the cursor → column-of-table lookup.
  // Captures both `dbo.` (schema-qualified column trigger) and `Emp.`
  // (bare table/alias). The provider tries to match the qualifier against
  // any known table name; if none, it just returns nothing (empty list is
  // better than a spurious keyword flood after a dot).
  const dotMatch = upToCursor.match(/([A-Za-z_][\w]*)\s*\.\s*$/);
  if (dotMatch && dotMatch[1]) {
    return { kind: 'column-of', qualifier: dotMatch[1] };
  }

  // Trailing keyword indicates the cursor is in a table slot. Only look at
  // the last uppercase keyword-shaped token in the buffer so extra spaces
  // and newlines don't throw it off.
  const tokens = upToCursor
    .toUpperCase()
    .split(/[^A-Z_]+/)
    .filter(Boolean);
  const last = tokens[tokens.length - 1];
  if (last === 'FROM' || last === 'JOIN' || last === 'INTO' || last === 'UPDATE') {
    return { kind: 'table-slot' };
  }
  // Multi-word joins — last token would be JOIN alone, already handled above.

  return { kind: 'default' };
}

// --- suggestion builders -----------------------------------------------------

interface Range {
  startLineNumber: number;
  endLineNumber: number;
  startColumn: number;
  endColumn: number;
}

function pushStatic(
  out: unknown[],
  items: StaticCompletion[],
  kind: number,
  sortPrefix: string,
  range: Range,
): void {
  for (const item of items) {
    out.push({
      label: item.label,
      kind,
      detail: item.detail,
      documentation: item.documentation,
      insertText: item.label,
      sortText: sortPrefix + item.label,
      range,
    });
  }
}

function pushTables(out: unknown[], K: { Class: number }, range: Range): void {
  for (const t of currentSnapshot.tables) {
    // Three surface forms — user might use any. Same insertText payload but
    // different labels so Monaco shows all three in the popup.
    const bare = t.name;
    const dotted = `${t.schema}.${t.name}`;
    const bracketed = `[${t.schema}].[${t.name}]`;

    out.push({
      label: bare,
      kind: K.Class,
      detail: `table (${t.schema})`,
      insertText: bare,
      sortText: SORT_TABLE + '0' + bare,
      range,
    });
    out.push({
      label: dotted,
      kind: K.Class,
      detail: 'table',
      insertText: dotted,
      sortText: SORT_TABLE + '1' + dotted,
      range,
    });
    out.push({
      label: bracketed,
      kind: K.Class,
      detail: 'table (bracketed)',
      insertText: bracketed,
      sortText: SORT_TABLE + '2' + bracketed,
      range,
    });
  }
}

function pushColumns(out: unknown[], K: { Field: number }, range: Range): void {
  for (const c of currentSnapshot.allColumns) {
    out.push({
      label: c.name,
      kind: K.Field,
      detail: c.type ? `${c.type} column` : 'column',
      insertText: c.name,
      sortText: SORT_COLUMN + c.name,
      range,
    });
  }
}

// Resolve `qualifier` (a bare identifier the user typed before the dot) to
// a specific table's columns. Case-insensitive so `emp.` matches `Employees`.
// If nothing matches — no columns; the empty list is more honest than
// flooding the popup with every column in the DB.
function columnsFor(qualifier: string): SqlSchemaSnapshot['allColumns'] {
  const q = qualifier.toLowerCase();
  for (const t of currentSnapshot.tables) {
    if (t.name.toLowerCase() === q) return t.columns;
  }
  return [];
}
