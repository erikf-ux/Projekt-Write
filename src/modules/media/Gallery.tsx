/**
 * The media gallery (section 17: "Galerie, virtualisiert, mit
 * Vorschaubildern aus `thumbnails/`"). Only the rows in view exist in the
 * DOM. One tab stop: arrow keys move, Enter opens. Thumbnails come from the
 * media handler; an SVG shows itself (as an image, never in the DOM); a PDF
 * without a thumbnail gets one drawn from page 1, one at a time.
 */
import { FileUp, FolderUp, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { flushSync } from 'react-dom';
import { useLocale, useT } from '@/i18n';
import { MediaIcon } from '@/modules/documents/Explorer';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import type { MediaKind } from '@/types/generated/MediaKind';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import { useMedia } from './mediaStore';
import { mediaUrl } from './mediaUrl';

const TILE_WIDTH = 168;
const TILE_HEIGHT = 168;
const OVERSCAN_ROWS = 2;

type KindFilter = 'all' | MediaKind;

/** PDFs waiting for a thumbnail, drawn one after the other. */
const pending: string[] = [];
let drawing = false;

function queueThumbnail(id: string): void {
  if (pending.includes(id)) {
    return;
  }
  pending.push(id);
  if (!drawing) {
    void drawNext();
  }
}

async function drawNext(): Promise<void> {
  const id = pending.shift();
  if (!id) {
    drawing = false;
    return;
  }
  drawing = true;
  try {
    // PDF.js loads only when a thumbnail is missing.
    const { renderPdfThumbnail } = await import('./pdf');
    const { png, pages } = await renderPdfThumbnail(id);
    await useMedia.getState().storeThumbnail(id, png);
    await useMedia.getState().setPageCount(id, pages);
  } catch {
    // A PDF that does not render keeps its icon.
  }
  await drawNext();
}

function Thumbnail({ medium, version }: { medium: MediaSummary; version: number }) {
  const [failed, setFailed] = useState(false);
  const [shownVersion, setShownVersion] = useState(version);
  if (version !== shownVersion) {
    setShownVersion(version);
    setFailed(false);
  }
  if (medium.kind === 'attachment' || failed) {
    return (
      <span className="flex size-full items-center justify-center" aria-hidden="true">
        <MediaIcon kind={medium.kind} />
      </span>
    );
  }
  const src =
    medium.mimeType === 'image/svg+xml' ? mediaUrl(medium.id) : mediaUrl(medium.id, 'thumbnail', version);
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      decoding="async"
      draggable={false}
      className="size-full object-contain"
      onError={() => {
        setFailed(true);
        if (medium.kind === 'pdf') {
          queueThumbnail(medium.id);
        }
      }}
    />
  );
}

export function Gallery({ hidden }: { hidden: boolean }) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const media = useMedia((state) => state.media);
  const versions = useMedia((state) => state.thumbnailVersion);
  const openMedia = useWorkspace((state) => state.openMedia);
  const [kind, setKind] = useState<KindFilter>('all');
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const scroller = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ width: 800, top: 0, height: 600 });

  const shown = useMemo(() => {
    const text = query.trim().toLocaleLowerCase(locale);
    const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
    return media
      .filter((medium) => medium.archivedAt === null)
      .filter((medium) => kind === 'all' || medium.kind === kind)
      .filter((medium) => text === '' || medium.title.toLocaleLowerCase(locale).includes(text))
      .sort((a, b) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id));
  }, [media, kind, query, locale]);

  useEffect(() => {
    const element = scroller.current;
    if (!element) {
      return;
    }
    const update = () => {
      flushSync(() => {
        setView({ width: element.clientWidth, top: element.scrollTop, height: element.clientHeight || 600 });
      });
    };
    update();
    element.addEventListener('scroll', update, { passive: true });
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null;
    observer?.observe(element);
    return () => {
      element.removeEventListener('scroll', update);
      observer?.disconnect();
    };
  }, [hidden]);

  const columns = Math.max(1, Math.floor(view.width / TILE_WIDTH));
  const rows = Math.ceil(shown.length / columns);
  const firstRow = Math.max(0, Math.floor(view.top / TILE_HEIGHT) - OVERSCAN_ROWS);
  const lastRow = Math.min(rows, Math.ceil((view.top + view.height) / TILE_HEIGHT) + OVERSCAN_ROWS);
  const visible = shown.slice(firstRow * columns, lastRow * columns);
  const current = Math.min(selected, Math.max(0, shown.length - 1));
  const tileId = (index: number) => `pw-gallery-${String(index)}`;

  // Keep the selected tile in view (and rendered).
  useEffect(() => {
    const element = scroller.current;
    if (!element || shown.length === 0) {
      return;
    }
    const top = Math.floor(current / columns) * TILE_HEIGHT;
    if (top < element.scrollTop) {
      element.scrollTop = top;
    } else if (top + TILE_HEIGHT > element.scrollTop + element.clientHeight) {
      element.scrollTop = top + TILE_HEIGHT - element.clientHeight;
    }
  }, [current, columns, shown.length]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const page = Math.max(1, Math.floor(view.height / TILE_HEIGHT)) * columns;
    const move: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: columns,
      ArrowUp: -columns,
      PageDown: page,
      PageUp: -page,
    };
    if (event.key in move) {
      setSelected(Math.min(shown.length - 1, Math.max(0, current + (move[event.key] ?? 0))));
    } else if (event.key === 'Home') {
      setSelected(0);
    } else if (event.key === 'End') {
      setSelected(shown.length - 1);
    } else if (event.key === 'Enter' && shown[current]) {
      openMedia(shown[current].id);
    } else {
      return;
    }
    event.preventDefault();
  };

  const filters: KindFilter[] = ['all', 'image', 'pdf', 'attachment'];
  const button = 'text-ui inline-flex h-7 items-center gap-1.5 rounded-pw px-2 hover:bg-surface';
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid="gallery">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-muted/40 px-3 py-2">
        <h2 className="text-ui mr-2 font-semibold">{t('media.gallery.title')}</h2>
        <div role="radiogroup" aria-label={t('media.gallery.filter')} className="flex items-center gap-1">
          {filters.map((filter) => (
            <button
              key={filter}
              type="button"
              role="radio"
              aria-checked={kind === filter}
              data-testid={`gallery-filter-${filter}`}
              className={`${button} ${kind === filter ? 'bg-surface text-accent' : 'text-fg'}`}
              onClick={() => {
                setKind(filter);
                setSelected(0);
              }}
            >
              {t(`media.gallery.filter.${filter}`)}
            </button>
          ))}
        </div>
        <label className="text-ui flex items-center gap-1 rounded-pw border border-muted/60 bg-surface px-1">
          <Search aria-hidden="true" className="size-3.5 text-muted" />
          <span className="sr-only">{t('media.gallery.search')}</span>
          <input
            className="h-6 w-44 bg-transparent text-fg outline-none"
            placeholder={t('media.gallery.search')}
            value={query}
            data-testid="gallery-search"
            onChange={(event) => {
              setQuery(event.target.value);
              setSelected(0);
            }}
          />
        </label>
        <span className="text-ui text-muted" data-testid="gallery-count">
          {t('media.gallery.count', { count: shown.length })}
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            className={button}
            data-testid="gallery-import"
            onClick={() => void useMedia.getState().startImport('files', t('media.import.title'))}
          >
            <FileUp aria-hidden="true" className="size-4" />
            {t('command.media.import')}
          </button>
          <button
            type="button"
            className={button}
            data-testid="gallery-import-folder"
            onClick={() => void useMedia.getState().startImport('folder', t('media.import.titleFolder'))}
          >
            <FolderUp aria-hidden="true" className="size-4" />
            {t('command.media.importFolder')}
          </button>
        </div>
      </div>
      <div
        ref={scroller}
        role="listbox"
        aria-label={t('media.gallery.title')}
        aria-activedescendant={shown.length > 0 ? tileId(current) : undefined}
        tabIndex={0}
        className="relative min-h-0 flex-1 overflow-auto outline-none focus-visible:ring-2 focus-visible:ring-accent"
        data-testid="gallery-list"
        onKeyDown={onKeyDown}
      >
        {shown.length === 0 ? (
          <p className="text-ui p-6 text-muted">
            {media.length === 0 ? t('media.gallery.empty') : t('media.gallery.noMatch')}
          </p>
        ) : null}
        <div style={{ height: rows * TILE_HEIGHT }} className="relative">
          {visible.map((medium, offset) => {
            const index = firstRow * columns + offset;
            const row = Math.floor(index / columns);
            const column = index % columns;
            return (
              <div
                key={medium.id}
                id={tileId(index)}
                role="option"
                aria-selected={index === current}
                aria-label={`${medium.title}, ${t(`media.kind.${medium.kind}`)}`}
                data-media={medium.id}
                data-media-kind={medium.kind}
                className={`absolute flex flex-col gap-1 rounded-pw p-2 ${
                  index === current ? 'bg-surface outline-2 outline-accent' : 'hover:bg-surface'
                }`}
                style={{
                  left: column * TILE_WIDTH,
                  top: row * TILE_HEIGHT,
                  width: TILE_WIDTH - 8,
                  height: TILE_HEIGHT - 8,
                }}
                onClick={() => {
                  setSelected(index);
                  // The arrows and Enter work on from the clicked tile
                  // (WebKit does not always focus the list on a click).
                  scroller.current?.focus({ preventScroll: true });
                }}
                onDoubleClick={() => {
                  openMedia(medium.id);
                }}
              >
                <div className="min-h-0 flex-1 overflow-hidden rounded-pw bg-sidebar">
                  <Thumbnail medium={medium} version={versions[medium.id] ?? 0} />
                </div>
                <span className="text-ui flex items-center gap-1 truncate">
                  <MediaIcon kind={medium.kind} />
                  <span className="truncate">{medium.title}</span>
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
