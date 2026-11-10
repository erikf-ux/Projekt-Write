/**
 * A PDF in Project Write (section 17): page by page, with zoom, a text
 * layer to select and copy text, and links. Web links open only after the
 * user agreed, through Rust's link check (http, https, mailto); links
 * inside the PDF jump to their page.
 */
import { ChevronLeft, ChevronRight, MoveHorizontal, ZoomIn, ZoomOut } from 'lucide-react';
import { TextLayer, type PDFDocumentProxy, type PDFPageProxy, type RenderTask } from 'pdfjs-dist';
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { ask } from '@/components/prompt/promptStore';
import { useT } from '@/i18n';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import { loadPdf, renderPdfThumbnail } from './pdf';
import { useMedia } from './mediaStore';
import './pdf.css';

const ZOOM_STEPS = [0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];

interface PdfViewProps {
  mediaId: string;
  /** The page to show; changes when a reference opens the PDF at a page. */
  page: number | null;
  /** Whether the PDF has a thumbnail already (else one is drawn). */
  needsThumbnail: boolean;
}

interface Link {
  key: string;
  left: number;
  top: number;
  width: number;
  height: number;
  url: string | null;
  page: number | null;
}

/** Asks, then opens a web link of a PDF through Rust's link check. */
async function openWebLink(url: string): Promise<void> {
  const answer = await ask({
    titleKey: 'pdf.linkTitle',
    textKey: 'pdf.linkText',
    params: { url },
    choices: [
      { id: 'open', labelKey: 'pdf.linkOpen', tone: 'primary' },
      { id: 'cancel', labelKey: 'media.cancel' },
    ],
    cancel: 'cancel',
    testId: 'pdf-link-prompt',
  });
  if (answer !== 'open') {
    return;
  }
  const result = await call('link_open', { url });
  if (!result.ok) {
    useUi.getState().showNotice({ tone: 'error', key: 'pdf.linkRefused', error: result.error });
  }
}

async function pageOfDestination(pdf: PDFDocumentProxy, dest: unknown): Promise<number | null> {
  try {
    const explicit = typeof dest === 'string' ? await pdf.getDestination(dest) : dest;
    if (!Array.isArray(explicit) || explicit.length === 0) {
      return null;
    }
    const target: unknown = explicit[0];
    if (typeof target === 'number') {
      return target + 1;
    }
    if (target && typeof target === 'object') {
      return (await pdf.getPageIndex(target as Parameters<PDFDocumentProxy['getPageIndex']>[0])) + 1;
    }
  } catch {
    // A broken destination goes nowhere.
  }
  return null;
}

export function PdfView({ mediaId, page: requested, needsThumbnail }: PdfViewProps) {
  const t = useT();
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null);
  const [failed, setFailed] = useState(false);
  const [page, setPage] = useState(requested ?? 1);
  const [zoom, setZoom] = useState<number | 'width'>('width');
  const [scale, setScale] = useState(1);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [links, setLinks] = useState<Link[]>([]);
  const [pageInput, setPageInput] = useState(String(requested ?? 1));
  const scroller = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const textLayer = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  // A reference to another page of this PDF.
  const [lastRequested, setLastRequested] = useState(requested);
  if (requested !== lastRequested) {
    setLastRequested(requested);
    if (requested) {
      setPage(requested);
      setPageInput(String(requested));
    }
  }

  useEffect(() => {
    let cancelled = false;
    let loaded: PDFDocumentProxy | null = null;
    loadPdf(mediaId).then(
      (document) => {
        if (cancelled) {
          void document.loadingTask.destroy();
          return;
        }
        loaded = document;
        setPdf(document);
        void useMedia.getState().setPageCount(mediaId, document.numPages);
      },
      () => {
        if (!cancelled) {
          setFailed(true);
        }
      },
    );
    return () => {
      cancelled = true;
      void loaded?.loadingTask.destroy();
    };
  }, [mediaId]);

  // The gallery shows page 1 once the PDF view has drawn it.
  useEffect(() => {
    if (!pdf || !needsThumbnail) {
      return;
    }
    void renderPdfThumbnail(mediaId).then(
      ({ png }) => useMedia.getState().storeThumbnail(mediaId, png),
      () => undefined,
    );
  }, [pdf, mediaId, needsThumbnail]);

  useEffect(() => {
    const element = scroller.current;
    if (!element || typeof ResizeObserver !== 'function') {
      return;
    }
    const observer = new ResizeObserver(() => {
      setWidth(element.clientWidth);
    });
    observer.observe(element);
    setWidth(element.clientWidth);
    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    if (!pdf || !canvas.current || !textLayer.current) {
      return;
    }
    // An object, so the cleanup below can flip it while a page draws.
    const run = { cancelled: false };
    const cancelled = () => run.cancelled;
    let task: RenderTask | null = null;
    let text: TextLayer | null = null;
    const draw = async () => {
      const current: PDFPageProxy = await pdf.getPage(Math.min(Math.max(1, page), pdf.numPages));
      if (cancelled() || !canvas.current || !textLayer.current) {
        return;
      }
      const unscaled = current.getViewport({ scale: 1 });
      const nextScale = zoom === 'width' ? Math.max(0.25, (width - 32) / unscaled.width) : zoom;
      const viewport = current.getViewport({ scale: nextScale });
      const ratio = window.devicePixelRatio || 1;
      const element = canvas.current;
      element.width = Math.floor(viewport.width * ratio);
      element.height = Math.floor(viewport.height * ratio);
      setScale(nextScale);
      setSize({ width: viewport.width, height: viewport.height });
      task = current.render({
        canvas: element,
        viewport,
        transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
        background: '#ffffff',
      });
      await task.promise;
      if (cancelled()) {
        return;
      }
      textLayer.current.replaceChildren();
      text = new TextLayer({
        textContentSource: current.streamTextContent(),
        container: textLayer.current,
        viewport,
      });
      await text.render();
      const annotations = (await current.getAnnotations()) as {
        subtype?: string;
        rect?: number[];
        url?: string;
        dest?: unknown;
        id?: string;
      }[];
      const found: Link[] = [];
      for (const [index, annotation] of annotations.entries()) {
        if (annotation.subtype !== 'Link' || !annotation.rect || annotation.rect.length !== 4) {
          continue;
        }
        const [rx1 = 0, ry1 = 0, rx2 = 0, ry2 = 0] = annotation.rect;
        const [x1, y1] = viewport.convertToViewportPoint(rx1, ry1) as [number, number];
        const [x2, y2] = viewport.convertToViewportPoint(rx2, ry2) as [number, number];
        const linkPage = annotation.url ? null : await pageOfDestination(pdf, annotation.dest);
        if (!annotation.url && linkPage === null) {
          continue;
        }
        found.push({
          key: annotation.id ?? String(index),
          left: Math.min(x1, x2),
          top: Math.min(y1, y2),
          width: Math.abs(x2 - x1),
          height: Math.abs(y2 - y1),
          url: annotation.url ?? null,
          page: linkPage,
        });
      }
      if (!cancelled()) {
        setLinks(found);
      }
    };
    draw().catch(() => {
      // A cancelled render or a damaged page: the toolbar stays usable.
    });
    return () => {
      run.cancelled = true;
      task?.cancel();
      text?.cancel();
    };
  }, [pdf, page, zoom, width]);

  const count = pdf?.numPages ?? 0;
  const go = (next: number) => {
    if (count === 0) {
      return;
    }
    const clamped = Math.min(Math.max(1, next), count);
    setPage(clamped);
    setPageInput(String(clamped));
    scroller.current?.scrollTo({ top: 0 });
  };
  const zoomBy = (direction: 1 | -1) => {
    const current = zoom === 'width' ? scale : zoom;
    const next =
      direction === 1
        ? (ZOOM_STEPS.find((step) => step > current + 0.001) ?? current)
        : ([...ZOOM_STEPS].reverse().find((step) => step < current - 0.001) ?? current);
    setZoom(next);
  };

  if (failed) {
    return (
      <p className="text-ui p-6 text-muted" role="alert" data-testid="pdf-failed">
        {t('pdf.failed')}
      </p>
    );
  }

  const button =
    'inline-flex size-7 items-center justify-center rounded-pw text-fg hover:bg-surface disabled:text-muted';
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid="pdf-view" data-page={page} data-pages={count}>
      <div
        role="toolbar"
        aria-label={t('pdf.toolbar')}
        className="flex h-9 shrink-0 items-center gap-1 border-b border-muted/40 bg-sidebar px-2"
      >
        <button
          type="button"
          className={button}
          aria-label={t('pdf.previous')}
          title={t('pdf.previous')}
          disabled={page <= 1}
          data-testid="pdf-previous"
          onClick={() => {
            go(page - 1);
          }}
        >
          <ChevronLeft aria-hidden="true" className="size-4" />
        </button>
        <label className="text-ui flex items-center gap-1">
          <span className="sr-only">{t('pdf.page')}</span>
          <input
            className="h-6 w-12 rounded-pw border border-muted/60 bg-surface px-1 text-right text-fg"
            inputMode="numeric"
            value={pageInput}
            data-testid="pdf-page"
            onChange={(event) => {
              setPageInput(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                go(Number(pageInput) || page);
              }
            }}
            onBlur={() => {
              go(Number(pageInput) || page);
            }}
          />
          <span className="text-muted">{t('pdf.of', { count })}</span>
        </label>
        <button
          type="button"
          className={button}
          aria-label={t('pdf.next')}
          title={t('pdf.next')}
          disabled={page >= count}
          data-testid="pdf-next"
          onClick={() => {
            go(page + 1);
          }}
        >
          <ChevronRight aria-hidden="true" className="size-4" />
        </button>
        <span className="mx-2 h-5 w-px bg-muted/40" aria-hidden="true" />
        <button
          type="button"
          className={button}
          aria-label={t('pdf.zoomOut')}
          title={t('pdf.zoomOut')}
          onClick={() => {
            zoomBy(-1);
          }}
        >
          <ZoomOut aria-hidden="true" className="size-4" />
        </button>
        <span className="text-ui w-14 text-center text-muted" data-testid="pdf-zoom">
          {t('pdf.zoom', { percent: Math.round(scale * 100) })}
        </span>
        <button
          type="button"
          className={button}
          aria-label={t('pdf.zoomIn')}
          title={t('pdf.zoomIn')}
          onClick={() => {
            zoomBy(1);
          }}
        >
          <ZoomIn aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={`${button} ${zoom === 'width' ? 'text-accent' : ''}`}
          aria-label={t('pdf.fitWidth')}
          aria-pressed={zoom === 'width'}
          title={t('pdf.fitWidth')}
          onClick={() => {
            setZoom('width');
          }}
        >
          <MoveHorizontal aria-hidden="true" className="size-4" />
        </button>
      </div>
      <div
        ref={scroller}
        className="min-h-0 flex-1 overflow-auto bg-sidebar p-4"
        tabIndex={0}
        aria-label={t('pdf.pageLabel', { page, count })}
        onKeyDown={(event) => {
          if (event.key === 'PageDown' && !event.ctrlKey) {
            go(page + 1);
            event.preventDefault();
          } else if (event.key === 'PageUp' && !event.ctrlKey) {
            go(page - 1);
            event.preventDefault();
          } else if ((event.ctrlKey || event.metaKey) && (event.key === '+' || event.key === '=')) {
            zoomBy(1);
            event.preventDefault();
          } else if ((event.ctrlKey || event.metaKey) && event.key === '-') {
            zoomBy(-1);
            event.preventDefault();
          }
        }}
        onWheel={(event) => {
          if (event.ctrlKey) {
            zoomBy(event.deltaY < 0 ? 1 : -1);
            event.preventDefault();
          }
        }}
      >
        {!pdf ? <p className="text-ui text-muted">{t('pdf.loading')}</p> : null}
        <div
          className="pw-pdf-page relative mx-auto bg-white shadow"
          style={
            {
              width: size.width,
              height: size.height,
              '--scale-factor': scale,
              '--total-scale-factor': scale,
              '--user-unit': 1,
              '--scale-round-x': '1px',
              '--scale-round-y': '1px',
            } as CSSProperties
          }
        >
          <canvas ref={canvas} className="absolute inset-0 size-full" data-testid="pdf-canvas" />
          <div ref={textLayer} className="textLayer" data-testid="pdf-text" />
          {links.map((link) => (
            <button
              key={link.key}
              type="button"
              className="absolute cursor-pointer bg-transparent hover:bg-accent/15"
              style={{ left: link.left, top: link.top, width: link.width, height: link.height }}
              aria-label={
                link.url
                  ? t('pdf.linkLabel', { url: link.url })
                  : t('pdf.pageLabel', { page: link.page ?? 1, count })
              }
              data-pdf-link={link.url ?? `page:${String(link.page)}`}
              onClick={() => {
                if (link.url) {
                  void openWebLink(link.url);
                } else if (link.page) {
                  go(link.page);
                }
              }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
