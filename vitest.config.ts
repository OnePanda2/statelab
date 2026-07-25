import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

// Vitest + React Testing Library (§3.4). jsdom environment; a setup file stubs the
// canvas 2D context and ResizeObserver, which jsdom does not implement.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    globals: false,
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
