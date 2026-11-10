/**
 * Addresses of media (ADR 0019, decision 8).
 *
 * Documents always name a medium as `pwmedia://<id>`, a PDF page as
 * `pwmedia://<id>#page=<n>`. The webview reaches the media handler at
 * `pwmedia://localhost/<id>` – under Windows (WebView2) at
 * `http://pwmedia.localhost/<id>`, the form Tauri's `convertFileSrc` uses as
 * well. The interface never sees a path.
 */

/** Prefix of media in documents. */
export const MEDIA_PREFIX = 'pwmedia://';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

function onWindows(): boolean {
  return typeof navigator !== 'undefined' && /Windows|Android/.test(navigator.userAgent);
}

/**
 * Where the webview loads a medium or its thumbnail. `version` changes the
 * address after a thumbnail was made, so the image loads again.
 */
export function mediaUrl(id: string, variant?: 'thumbnail', version?: number): string {
  const base = onWindows() ? 'http://pwmedia.localhost/' : 'pwmedia://localhost/';
  const path = variant === 'thumbnail' ? `${id}/thumbnail` : id;
  return version ? `${base}${path}?v=${String(version)}` : `${base}${path}`;
}

/** `pwmedia://<id>`, with `#page=<n>` for a PDF page. */
export function mediaReference(id: string, page?: number | null): string {
  return page ? `${MEDIA_PREFIX}${id}#page=${String(page)}` : `${MEDIA_PREFIX}${id}`;
}

/** The medium (and page) of a reference, or `null` if it is none. */
export function parseMediaReference(reference: string): { id: string; page: number | null } | null {
  if (!reference.startsWith(MEDIA_PREFIX)) {
    return null;
  }
  const rest = reference.slice(MEDIA_PREFIX.length);
  const id = rest.slice(0, 36);
  if (!UUID.test(id)) {
    return null;
  }
  const tail = rest.slice(36);
  if (tail === '') {
    return { id, page: null };
  }
  const match = /^#page=(\d{1,6})$/.exec(tail);
  if (!match) {
    return null;
  }
  const page = Number(match[1]);
  return { id, page: page >= 1 && page <= 100_000 ? page : null };
}
