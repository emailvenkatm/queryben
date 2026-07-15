export type ScriptObjectKind = 'table' | 'view' | 'procedure' | 'function' | 'index';

export type ScriptAction = 'create' | 'alter' | 'drop' | 'dropAndCreate' | 'selectTop' | 'insertTemplate';

export interface ScriptObjectArgs {
  connectionId: string;
  kind: ScriptObjectKind;
  schema: string;
  name: string;
  // Only used for indexes (parent table). Other kinds ignore it.
  table: string | null;
  action: ScriptAction;
}
