import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

// Tauri expects a fixed port during dev; 1420 is the Tauri default.
// TAURI_DEV_HOST is set by the Tauri CLI when running on a physical device.
const host = process.env['TAURI_DEV_HOST'];

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  // Prevent Vite from obscuring Rust panic messages
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Watch src-tauri so Tauri CLI can react to Rust changes
      ignored: ['**/src-tauri/**'],
    },
  },
  // Monaco Editor ships its own workers; we don't want Vite to process them
  optimizeDeps: {
    exclude: ['@monaco-editor/react'],
  },
  build: {
    // Tauri uses Chromium on macOS/Windows, Safari engine on iOS
    target: ['es2021', 'chrome105', 'safari15'],
    minify: !process.env['TAURI_DEBUG'] ? 'esbuild' : false,
    sourcemap: !!process.env['TAURI_DEBUG'],
    rollupOptions: {
      output: {
        manualChunks: {
          'monaco-editor': ['@monaco-editor/react'],
          'tanstack-table': ['@tanstack/react-table'],
          'tanstack-query': ['@tanstack/react-query'],
        },
      },
    },
  },
});
