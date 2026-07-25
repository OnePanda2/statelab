import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { fileURLToPath, URL } from 'node:url';

// Standard Tauri + Vite pairing (§3.4). `viteSingleFile` inlines all JS/CSS into a
// single `dist/index.html`, which the `statelab-app` host embeds into the one
// double-click `.exe`. Fixed dev port so a future Tauri shell can attach.
export default defineConfig({
  plugins: [react(), viteSingleFile()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
