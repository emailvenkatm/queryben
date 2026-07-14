import { useEffect, useRef, useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { complete, newSession } from '../api';
import type { AiMessage } from '../types';

// Grabs the first ```sql fenced block; falls back to the first generic fence.
export function extractSqlBlock(text: string): string | undefined {
  const sql = /```sql\s*\n([\s\S]*?)```/i.exec(text);
  if (sql?.[1]) return sql[1].trim();
  const any = /```\s*\n([\s\S]*?)```/.exec(text);
  if (any?.[1]) return any[1].trim();
  return undefined;
}

interface Options {
  connectionId: string | null;
}

export function useAi({ connectionId }: Options) {
  const [messages, setMessages] = useState<AiMessage[]>([]);
  const [sessionError, setSessionError] = useState<string | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const sessionConnRef = useRef<string | null>(null);

  // When the user switches connections, drop the session so the next send
  // opens a fresh /newchat against the new schema.
  useEffect(() => {
    if (sessionConnRef.current && sessionConnRef.current !== connectionId) {
      sessionIdRef.current = null;
      sessionConnRef.current = null;
      setMessages([]);
    }
  }, [connectionId]);

  const sessionMutation = useMutation({
    mutationFn: (cid: string) => newSession(cid),
    onSuccess: (sid, cid) => {
      sessionIdRef.current = sid;
      sessionConnRef.current = cid;
      setSessionError(null);
    },
    onError: (err) => {
      const msg = err instanceof Error ? err.message : String(err);
      setSessionError(msg);
    },
  });

  const completeMutation = useMutation({
    mutationFn: ({ sid, prompt }: { sid: string; prompt: string }) =>
      complete(sid, prompt),
  });

  async function send(prompt: string) {
    if (!connectionId) return;
    const trimmed = prompt.trim();
    if (!trimmed) return;

    setMessages((prev) => [
      ...prev,
      { id: crypto.randomUUID(), role: 'user', content: trimmed, createdAt: Date.now() },
    ]);

    let sid = sessionIdRef.current;
    if (!sid || sessionConnRef.current !== connectionId) {
      try {
        sid = await sessionMutation.mutateAsync(connectionId);
      } catch {
        return;
      }
    }
    if (!sid) return;

    try {
      const reply = await completeMutation.mutateAsync({ sid, prompt: trimmed });
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: reply,
          sqlBlock: extractSqlBlock(reply),
          createdAt: Date.now(),
        },
      ]);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setMessages((prev) => [
        ...prev,
        { id: crypto.randomUUID(), role: 'assistant', content: `Error: ${msg}`, createdAt: Date.now() },
      ]);
    }
  }

  function reset() {
    sessionIdRef.current = null;
    sessionConnRef.current = null;
    setMessages([]);
    setSessionError(null);
  }

  return {
    messages,
    send,
    reset,
    isPending: sessionMutation.isPending || completeMutation.isPending,
    sessionError,
  };
}
