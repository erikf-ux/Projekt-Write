/**
 * "Bild einfügen …" and "PDF-Verweis einfügen …" (section 12; ADR 0019,
 * decision 10): choose a medium of the library – or import a new one – and
 * insert it at the cursor. An image becomes `![Alt](pwmedia://<id>)`, a PDF
 * reference a link `[Text](pwmedia://<id>#page=N)`; with text selected,
 * the reference links that text.
 */
import { useEffect, useId, useMemo, useState, type KeyboardEvent } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { useLocale, useT } from '@/i18n';
import { useMedia } from '@/modules/media/mediaStore';
import { mediaUrl } from '@/modules/media/mediaUrl';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import { getSession } from './session';

/** Most media shown at once; the search narrows the rest. */
const MAX_SHOWN = 50;

export type PickerMode = 'image' | 'pdf';

interface MediaPickerStore {
  request: { tab: string; mode: PickerMode } | null;
  open: (tab: string, mode: PickerMode) => void;
  close: () => void;
}

export const useMediaPicker = create<MediaPickerStore>((set) => ({
  request: null,
  open: (tab, mode) => {
    set({ request: { tab, mode } });
  },
  close: () => {
    set({ request: null });
  },
}));

/** Media of the kind whose title contains every word of the query, by title. */
export function matchingMedia(
  media: readonly MediaSummary[],
  mode: PickerMode,
  query: string,
  locale: string,
): MediaSummary[] {
  const words = query.toLocaleLowerCase(locale).split(/\s+/).filter(Boolean);
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  return media
    .filter((medium) => medium.kind === mode && medium.archivedAt === null && medium.deletedAt === null)
    .filter((medium) => {
      const title = medium.title.toLocaleLowerCase(locale);
      return words.every((word) => title.includes(word));
    })
    .sort((a, b) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id))
    .slice(0, MAX_SHOWN);
}

/** The page of the field: null for the whole PDF, NaN if it is not valid. */
export function pageOf(text: string, pageCount: number | null): number | null {
  const value = text.trim();
  if (value === '') {
    return null;
  }
  if (!/^\d{1,6}$/.test(value)) {
    return Number.NaN;
  }
  const page = Number(value);
  return page >= 1 && (pageCount === null || page <= pageCount) ? page : Number.NaN;
}

function optionId(id: string): string {
  return `pw-media-option-${id}`;
}

export function MediaPicker() {
  const request = useMediaPicker((state) => state.request);
  return request ? (
    <Picker key={`${request.tab}:${request.mode}`} tab={request.tab} mode={request.mode} />
  ) : null;
}

function Picker({ tab, mode }: { tab: string; mode: PickerMode }) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const media = useMedia((state) => state.media);
  const lastImport = useMedia((state) => state.lastImport);
  const importing = useMedia((state) => state.importing);
  const [query, setQuery] = useState('');
  const [chosen, setChosen] = useState<string | null>(null);
  const [pageText, setPageText] = useState('');
  const [linkText, setLinkText] = useState('');
  const [touched, setTouched] = useState(false);
  const [selectionEmpty] = useState(() => getSession(tab)?.editor.state.selection.empty ?? true);
  const [importSeen, setImportSeen] = useState(lastImport);
  const listId = useId();
  const pageId = useId();
  const textId = useId();

  const shown = useMemo(() => matchingMedia(media, mode, query, locale), [media, mode, query, locale]);
  // What was imported from here is chosen (the first of the right kind).
  if (lastImport !== importSeen) {
    setImportSeen(lastImport);
    const imported = lastImport?.items
      .map((item) => media.find((medium) => medium.id === item.id))
      .find((medium) => medium?.kind === mode);
    if (imported) {
      setQuery('');
      setChosen(imported.id);
    }
  }
  const current = shown.find((medium) => medium.id === chosen) ?? shown[0];
  const page = mode === 'pdf' ? pageOf(pageText, current?.pageCount ?? null) : null;
  const pageInvalid = page !== null && Number.isNaN(page);

  useEffect(() => {
    if (current) {
      document.getElementById(optionId(current.id))?.scrollIntoView({ block: 'nearest' });
    }
  }, [current]);

  const close = () => {
    useMediaPicker.getState().close();
    getSession(tab)?.editor.commands.focus();
  };
  const insert = () => {
    setTouched(true);
    if (!current || pageInvalid) {
      return;
    }
    const session = getSession(tab);
    useMediaPicker.getState().close();
    if (mode === 'image') {
      session?.insertMedia([{ kind: 'image', id: current.id, alt: current.description || current.title }]);
      return;
    }
    const pageLabel = page === null ? '' : ` (${t('media.view.page', { page })})`;
    session?.insertMedia([
      {
        kind: 'link',
        id: current.id,
        text: linkText.trim() || `${current.title}${pageLabel}`,
        page,
      },
    ]);
  };
  const move = (step: number) => {
    const index = current ? shown.indexOf(current) : -1;
    const next = shown[Math.min(shown.length - 1, Math.max(0, index + step))];
    if (next) {
      setChosen(next.id);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      move(1);
    } else if (event.key === 'ArrowUp') {
      move(-1);
    } else if (event.key === 'Enter') {
      insert();
    } else {
      return;
    }
    event.preventDefault();
  };
  return (
    <Dialog
      title={t(mode === 'image' ? 'mediaPicker.titleImage' : 'mediaPicker.titlePdf')}
      onClose={close}
      testId="media-picker"
      width="w-[min(560px,92vw)]"
    >
      <form
        className="text-ui flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          insert();
        }}
      >
        <input
          type="text"
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-activedescendant={current ? optionId(current.id) : undefined}
          aria-label={t('mediaPicker.search')}
          placeholder={t('mediaPicker.search')}
          className="h-8 w-full rounded-pw border border-muted bg-surface px-2 text-fg"
          value={query}
          autoFocus
          data-autofocus
          data-testid="media-picker-search"
          onChange={(event) => {
            setQuery(event.target.value);
            setChosen(null);
          }}
          onKeyDown={onKeyDown}
        />
        <ul
          id={listId}
          role="listbox"
          aria-label={t('mediaPicker.list')}
          className="max-h-64 overflow-y-auto"
          data-testid="media-picker-list"
        >
          {shown.map((medium) => (
            <li
              key={medium.id}
              id={optionId(medium.id)}
              role="option"
              aria-selected={medium.id === current?.id}
              data-media={medium.id}
              className={`flex cursor-default items-center gap-2 rounded-pw px-2 py-1 ${
                medium.id === current?.id ? 'bg-surface outline-2 outline-accent' : ''
              }`}
              onMouseDown={(event) => {
                event.preventDefault();
                setChosen(medium.id);
              }}
              onDoubleClick={insert}
            >
              <img
                src={mediaUrl(medium.id, 'thumbnail')}
                alt=""
                className="size-8 shrink-0 rounded-pw bg-sidebar object-cover"
                draggable={false}
              />
              <span className="min-w-0 flex-1 truncate">{medium.title}</span>
              {medium.pageCount ? (
                <span className="text-muted">{t('mediaPicker.pages', { count: medium.pageCount })}</span>
              ) : null}
            </li>
          ))}
        </ul>
        {shown.length === 0 ? (
          <p className="text-muted" role="status">
            {t('mediaPicker.empty')}
          </p>
        ) : null}
        {mode === 'pdf' ? (
          <div className="flex flex-wrap gap-3">
            <label htmlFor={pageId} className="flex flex-col gap-1">
              {t('mediaPicker.page')}
              <input
                id={pageId}
                type="text"
                inputMode="numeric"
                className="h-8 w-32 rounded-pw border border-muted bg-surface px-2 text-fg aria-[invalid=true]:border-danger"
                value={pageText}
                aria-invalid={touched && pageInvalid}
                data-testid="media-picker-page"
                onChange={(event) => {
                  setPageText(event.target.value);
                }}
              />
              {touched && pageInvalid ? (
                <span className="text-danger" role="alert">
                  {t('mediaPicker.pageInvalid', { count: current?.pageCount ?? 1 })}
                </span>
              ) : null}
            </label>
            {selectionEmpty ? (
              <label htmlFor={textId} className="flex min-w-0 flex-1 flex-col gap-1">
                {t('mediaPicker.text')}
                <input
                  id={textId}
                  type="text"
                  className="h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
                  value={linkText}
                  placeholder={current?.title ?? ''}
                  data-testid="media-picker-text"
                  onChange={(event) => {
                    setLinkText(event.target.value);
                  }}
                />
              </label>
            ) : null}
          </div>
        ) : null}
        <div className="flex flex-wrap justify-between gap-2">
          <button
            type="button"
            className="h-7 rounded-pw border border-muted px-3 disabled:opacity-60"
            disabled={importing !== null}
            data-testid="media-picker-import"
            onClick={() => void useMedia.getState().startImport('files', t('media.import.title'))}
          >
            {t('mediaPicker.importNew')}
          </button>
          <div className="flex gap-2">
            <button type="button" className="h-7 rounded-pw border border-muted px-3" onClick={close}>
              {t('media.cancel')}
            </button>
            <button
              type="submit"
              className="h-7 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-60"
              disabled={!current}
              data-testid="media-picker-insert"
            >
              {t('mediaPicker.insert')}
            </button>
          </div>
        </div>
      </form>
    </Dialog>
  );
}
