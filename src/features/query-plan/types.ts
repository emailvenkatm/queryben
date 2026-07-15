export type OpKind =
  | 'tableScan'
  | 'indexScan'
  | 'indexSeek'
  | 'sort'
  | 'hashMatch'
  | 'nestedLoops'
  | 'mergeJoin'
  | 'computeScalar'
  | 'filter'
  | 'aggregate'
  | 'parallelism'
  | 'spool'
  | 'unknown';

export type WarningKind = 'missingIndex' | 'largeScan' | 'implicitConversion' | 'noJoinPredicate' | 'other';

export interface PlanWarning {
  kind: WarningKind;
  message: string;
}

export interface PlanNode {
  id: number;
  name: string;
  opKind: OpKind;
  estimatedRows: number | null;
  actualRows: number | null;
  estimatedCost: number | null;
  warnings: PlanWarning[];
  object: string | null;
  children: PlanNode[];
}

export interface QueryPlan {
  statementText: string | null;
  root: PlanNode;
  warnings: PlanWarning[];
  isActual: boolean;
}
