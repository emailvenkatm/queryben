import { invoke } from '@/shared/api/tauri';

// Local wrappers kept here so this feature ships without touching shared/api/tauri.ts
// while other agents are editing it.
export function newSession(connectionId: string): Promise<string> {
  return invoke<string>('ai_new_session', { connectionId });
}

export function complete(sessionId: string, prompt: string): Promise<string> {
  return invoke<string>('ai_complete', { sessionId, prompt });
}
