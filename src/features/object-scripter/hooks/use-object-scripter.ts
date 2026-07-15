import { useMutation } from '@tanstack/react-query';
import { scriptObject } from '../api';

// One-shot mutation — each "Script as" click is independent. No cache key
// because DDL reflects live metadata that changes between clicks.
export function useObjectScripter() {
  return useMutation({ mutationFn: scriptObject });
}
