import { invoke } from '@tauri-apps/api/core';
import type { ScriptObjectArgs } from './types';

// Direct invoke — parallel agents edit shared tauri-bindings.ts so this
// feature maintains its own surface.
export function scriptObject(args: ScriptObjectArgs): Promise<string> {
  return invoke('script_object', {
    connectionId: args.connectionId,
    kind: args.kind,
    schema: args.schema,
    name: args.name,
    table: args.table,
    action: args.action,
  });
}
