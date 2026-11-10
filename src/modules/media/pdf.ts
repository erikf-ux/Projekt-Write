/**
 * PDF.js in Project Write (section 17; ADR 0019, decision 9).
 *
 * * The file comes from the media handler (`fetch`), PDF.js gets its bytes.
 * * Worker, standard fonts, CMaps and the JavaScript fallbacks of the image
 *   decoders are part of the build (`/pdfjs/`, see `vite.config.ts`); nothing
 *   is loaded from the web.
 * * WebAssembly is off (the CSP has no 'wasm-unsafe-eval'), XFA is off, and
 *   PDF scripts never run: neither the viewer component nor its scripting
 *   sandbox are part of the app. PDF.js 6.5 has no `eval` path any more
 *   (the former option `isEvalSupported` is gone); the CSP forbids
 *   'unsafe-eval' as well.
 */
import { getDocument, GlobalWorkerOptions, type PDFDocumentProxy } from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import { mediaUrl } from './mediaUrl';

GlobalWorkerOptions.workerSrc = workerUrl;

/** Longest side of a PDF thumbnail (Rust accepts up to 512). */
export const PDF_THUMBNAIL_EDGE = 320;

function assetUrl(path: string): string {
  return new URL(`/pdfjs/${path}`, window.location.href).href;
}

/** Loads a PDF of the library. */
export async function loadPdf(mediaId: string): Promise<PDFDocumentProxy> {
  const response = await fetch(mediaUrl(mediaId));
  if (!response.ok) {
    throw new Error(`medium not served (${String(response.status)})`);
  }
  const data = new Uint8Array(await response.arrayBuffer());
  const task = getDocument({
    data,
    useWasm: false,
    enableXfa: false,
    useSystemFonts: false,
    standardFontDataUrl: assetUrl('standard_fonts/'),
    cMapUrl: assetUrl('cmaps/'),
    cMapPacked: true,
    wasmUrl: assetUrl('wasm/'),
    // Bomb guard for embedded images, like the import limit for images.
    maxImageSize: 100_000_000,
  });
  return task.promise;
}

/** Page 1 as PNG (Base64), at most [`PDF_THUMBNAIL_EDGE`] pixels on its longer side. */
export async function renderPdfThumbnail(mediaId: string): Promise<{ png: string; pages: number }> {
  const pdf = await loadPdf(mediaId);
  try {
    const page = await pdf.getPage(1);
    const unscaled = page.getViewport({ scale: 1 });
    const scale = PDF_THUMBNAIL_EDGE / Math.max(unscaled.width, unscaled.height);
    const viewport = page.getViewport({ scale });
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.floor(viewport.width));
    canvas.height = Math.max(1, Math.floor(viewport.height));
    await page.render({ canvas, viewport, background: '#ffffff' }).promise;
    const png = canvas.toDataURL('image/png').split(',')[1] ?? '';
    return { png, pages: pdf.numPages };
  } finally {
    void pdf.loadingTask.destroy();
  }
}
