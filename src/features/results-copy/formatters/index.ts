import type { FormatterRegistry } from '../types';
import { insertFormatter, targetFromSql } from './insert';
import { markdownFormatter } from './markdown';
import { jsonFormatter } from './json';
import { csvFormatter } from './csv';
import { cellValueFormatter } from './cell';

export { insertFormatter, targetFromSql } from './insert';
export { markdownFormatter } from './markdown';
export { jsonFormatter } from './json';
export { csvFormatter } from './csv';
export { cellValueFormatter } from './cell';

export const defaultRegistry: FormatterRegistry = {
  [insertFormatter.id]: insertFormatter,
  [markdownFormatter.id]: markdownFormatter,
  [jsonFormatter.id]: jsonFormatter,
  [csvFormatter.id]: csvFormatter,
  [cellValueFormatter.id]: cellValueFormatter,
};
