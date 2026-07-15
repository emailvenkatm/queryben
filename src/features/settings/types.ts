export interface UiPrefs {
  editorFontSize: number;
  editorWordWrap: boolean;
  connectionTimeoutSec: number;
  resultsMaxRows: number;
  autoUpdateEnabled: boolean;
}

export const DEFAULT_PREFS: UiPrefs = {
  editorFontSize: 13,
  editorWordWrap: false,
  connectionTimeoutSec: 30,
  resultsMaxRows: 1000,
  autoUpdateEnabled: true,
};
