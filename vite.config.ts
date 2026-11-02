import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Tauri expects a fixed dev server port and must be able to see Rust errors.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    // WebView2 (Chromium) on Windows, WebKit on macOS and Linux.
    target: ['es2022', 'chrome105', 'safari15'],
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    // No inline scripts or styles may be emitted: the CSP forbids them.
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
  },
});
