// Wire @monaco-editor/react to the locally bundled monaco-editor package.
// Without this it falls back to its default jsdelivr CDN loader, which hangs
// or times out after long idle periods (stale network, sleep-wake, offline)
// and blocks new query tabs on "Loading editor…" indefinitely.
import * as monaco from 'monaco-editor';
import { loader } from '@monaco-editor/react';
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
};

loader.config({ monaco });
