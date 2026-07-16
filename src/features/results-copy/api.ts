import type { CopyConfig } from './types';

export const defaultCopyConfig: CopyConfig = {
  insertQuoteIdentifiers: true,
  insertBracketIdentifiers: true,
  csvDelimiter: ',',
  markdownAlignNumbers: 'right',
  dateFormat: 'iso',
};

export function loadCopyConfig(): CopyConfig {
  return defaultCopyConfig;
}
