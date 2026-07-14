export { SchemaCompareScreen } from './components/schema-compare-screen';
export { DiffTree } from './components/DiffTree';
export { ObjectDiffPanel } from './components/ObjectDiffPanel';
export { MigrationSqlPanel } from './components/MigrationSqlPanel';

export {
  useSchemaSnapshot,
  useSchemaDiff,
  useSchemaDdl,
} from './hooks/use-schema-compare';

export type {
  ColumnSpec,
  DdlStatement,
  IndexSpec,
  ObjectChange,
  ObjectKind,
  SchemaDiff,
  SchemaObject,
  SchemaSnapshot,
} from './types';
