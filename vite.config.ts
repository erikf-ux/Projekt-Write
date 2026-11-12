import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';

/**
 * d3-color (through React Flow, Phase 11) sets `prototype.constructor` by
 * assignment. The app freezes `Object.prototype` (Tauri `freezePrototype`,
 * section 20), so that assignment throws in strict mode ("override
 * mistake") and the Mesh could not load. It is rewritten to define the
 * property, which a frozen prototype allows (ADR 0022). The build fails if
 * d3-color changes and the line is gone.
 */
function d3ColorFrozenPrototype(): Plugin {
  const line = 'prototype.constructor = constructor;';
  return {
    name: 'd3-color-frozen-prototype',
    transform(code, id) {
      if (!/[\\/]node_modules[\\/]d3-color[\\/]/.test(id)) {
        return null;
      }
      if (!code.includes(line)) {
        this.error(`d3-color changed: "${line}" not found in ${id}`);
      }
      return {
        code: code.replace(
          line,
          'Object.defineProperty(prototype, "constructor", { value: constructor, writable: true, configurable: true });',
        ),
        map: null,
      };
    },
  };
}

/**
 * Data files PDF.js loads at run time (ADR 0019, decision 9): standard
 * fonts, CMaps and the JavaScript fallbacks of the JPX and JBIG2 decoders
 * (WebAssembly stays off: the CSP does not allow 'wasm-unsafe-eval'). Served
 * under `/pdfjs/` by the dev server and copied to `dist/pdfjs/` when built,
 * so nothing is loaded from the web.
 */
function pdfjsAssets(): Plugin {
  const source = fileURLToPath(new URL('./node_modules/pdfjs-dist', import.meta.url));
  const wanted = (relative: string) =>
    relative.startsWith('standard_fonts/') ||
    relative.startsWith('cmaps/') ||
    (relative.startsWith('wasm/') &&
      (relative.endsWith('_nowasm_fallback.js') || relative.includes('LICENSE')));
  const types: Record<string, string> = {
    '.js': 'text/javascript',
    '.bcmap': 'application/octet-stream',
    '.pfb': 'application/octet-stream',
    '.ttf': 'font/ttf',
  };
  let outDir = 'dist';
  return {
    name: 'pdfjs-assets',
    configResolved(config) {
      outDir = path.resolve(config.root, config.build.outDir);
    },
    configureServer(server) {
      server.middlewares.use('/pdfjs/', (request, response, next) => {
        const relative = decodeURIComponent((request.url ?? '').split('?')[0] ?? '').replace(/^\/+/, '');
        const file = path.join(source, relative);
        if (!wanted(relative) || !file.startsWith(source + path.sep) || !fs.existsSync(file)) {
          next();
          return;
        }
        response.setHeader('Content-Type', types[path.extname(file)] ?? 'application/octet-stream');
        fs.createReadStream(file).pipe(response);
      });
    },
    writeBundle() {
      for (const dir of ['standard_fonts', 'cmaps', 'wasm']) {
        for (const name of fs.readdirSync(path.join(source, dir))) {
          const relative = `${dir}/${name}`;
          if (wanted(relative)) {
            fs.mkdirSync(path.join(outDir, 'pdfjs', dir), { recursive: true });
            fs.copyFileSync(path.join(source, relative), path.join(outDir, 'pdfjs', relative));
          }
        }
      }
    },
  };
}

// Tauri expects a fixed dev server port and must be able to see Rust errors.
export default defineConfig({
  plugins: [react(), tailwindcss(), pdfjsAssets(), d3ColorFrozenPrototype()],
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
