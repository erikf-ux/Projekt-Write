/**
 * The side bar "Suche" (sections 11 and 19): field, filters and the list of
 * hits with a snippet and the marked word. Arrow keys move through the
 * hits, Enter or a click opens one at the place it was found.
 */
import {
  Archive,
  FileText,
  Filter,
  Image,
  Network,
  Paperclip,
  Star,
  Trash2,
  X,
  FileType,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { errorText } from '@/components/errorText';
import { useLocale, useT, type MessageKey } from '@/i18n';
import { useStructure } from '@/modules/structure/structureStore';
import { buildIndex, inTreeOrder, pathLabel } from '@/modules/structure/tree';
import type { SearchHit } from '@/types/generated/SearchHit';
import type { SearchKind } from '@/types/generated/SearchKind';
import { openHit } from './openHit';
import { filtering, SEARCH_KINDS, SEARCH_SHOWN_MARK, useSearch } from './searchStore';

export const SEARCH_FIELD_ID = 'pw-search-field';
const LIST_ID = 'pw-search-results';
/** Tags offered in the filter list at a time. */
const TAG_CHOICES = 50;

const KIND_ICONS: Record<SearchKind, typeof FileText> = {
  document: FileText,
  image: Image,
  pdf: FileType,
  attachment: Paperclip,
  subnet: Network,
};

const KIND_LABELS: Record<SearchKind, MessageKey> = {
  document: 'search.kind.document',
  image: 'search.kind.image',
  pdf: 'search.kind.pdf',
  attachment: 'search.kind.attachment',
  subnet: 'search.kind.subnet',
};

function optionId(index: number): string {
  return `pw-search-hit-${String(index)}`;
}

const FIELD = 'text-ui h-7 min-w-0 rounded-pw border border-muted bg-surface px-2 text-fg';
const CHECK_LABEL = 'inline-flex items-center gap-1.5 text-fg';

function Snippet({ hit }: { hit: SearchHit }) {
  if (hit.snippet.length === 0) {
    return null;
  }
  return (
    <span className="line-clamp-2 text-muted" data-snippet="">
      {hit.snippet.map((part, index) =>
        part.hit ? (
          <mark key={index} className="rounded-sm bg-accent/25 px-0.5 text-fg">
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </span>
  );
}

function SearchFilters() {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const filters = useSearch((state) => state.filters);
  const setFilters = useSearch((state) => state.setFilters);
  const clearFilters = useSearch((state) => state.clearFilters);
  const subnets = useStructure((state) => state.subnets);
  const tags = useStructure((state) => state.tags);
  const [tagQuery, setTagQuery] = useState('');
  const index = useMemo(() => buildIndex(subnets, locale), [subnets, locale]);
  const subnetOptions = useMemo(
    () => inTreeOrder(index).map((subnet) => ({ id: subnet.id, label: pathLabel(index, subnet.id) })),
    [index],
  );
  const tagChoices = useMemo(() => {
    const needle = tagQuery.trim().toLocaleLowerCase(locale);
    return tags
      .filter((tag) => !filters.tagIds.includes(tag.id))
      .filter((tag) => needle === '' || tag.name.toLocaleLowerCase(locale).includes(needle))
      .slice(0, TAG_CHOICES);
  }, [tags, tagQuery, filters.tagIds, locale]);
  const tagName = (id: string) => tags.find((tag) => tag.id === id)?.name ?? '…';

  const toggleKind = (kind: SearchKind) => {
    const kinds = filters.kinds.includes(kind)
      ? filters.kinds.filter((entry) => entry !== kind)
      : [...filters.kinds, kind];
    setFilters({ kinds });
  };

  return (
    <div className="text-ui flex shrink-0 flex-col gap-2 px-3 pb-2" data-testid="search-filters">
      <fieldset className="flex flex-wrap gap-x-3 gap-y-1">
        <legend className="mb-1 text-muted">{t('search.filter.kind')}</legend>
        {SEARCH_KINDS.map((kind) => (
          <label key={kind} className={CHECK_LABEL}>
            <input
              type="checkbox"
              data-filter-kind={kind}
              checked={filters.kinds.includes(kind)}
              onChange={() => {
                toggleKind(kind);
              }}
            />
            {t(KIND_LABELS[kind])}
          </label>
        ))}
      </fieldset>
      <div className="flex flex-col gap-1">
        <span className="text-muted">{t('search.filter.tags')}</span>
        {filters.tagIds.length > 0 ? (
          <div className="flex flex-wrap gap-1">
            {filters.tagIds.map((id) => (
              <button
                key={id}
                type="button"
                data-filter-tag={id}
                className="inline-flex h-6 items-center gap-1 rounded-pw border border-accent px-1.5 text-fg"
                aria-label={t('search.filter.tagRemove', { tag: tagName(id) })}
                onClick={() => {
                  setFilters({ tagIds: filters.tagIds.filter((entry) => entry !== id) });
                }}
              >
                # {tagName(id)}
                <X aria-hidden="true" className="size-3" />
              </button>
            ))}
          </div>
        ) : null}
        <input
          type="search"
          className={FIELD}
          aria-label={t('search.filter.tagSearch')}
          placeholder={t('search.filter.tagSearch')}
          data-testid="search-filter-tag-search"
          disabled={tags.length === 0}
          value={tagQuery}
          onChange={(event) => {
            setTagQuery(event.target.value);
          }}
        />
        {tagQuery.trim() !== '' ? (
          <ul
            className="max-h-32 overflow-y-auto rounded-pw border border-muted p-1"
            data-testid="search-filter-tag-list"
          >
            {tagChoices.map((tag) => (
              <li key={tag.id}>
                <button
                  type="button"
                  data-tag-choice={tag.id}
                  className="w-full truncate rounded-pw px-1.5 py-0.5 text-left text-fg hover:bg-surface"
                  onClick={() => {
                    setFilters({ tagIds: [...filters.tagIds, tag.id] });
                    setTagQuery('');
                  }}
                >
                  # {tag.name}
                </button>
              </li>
            ))}
            {tagChoices.length === 0 ? (
              <li className="px-1.5 text-muted">{t('search.filter.noTags')}</li>
            ) : null}
          </ul>
        ) : null}
      </div>
      <label className="flex flex-col gap-1">
        <span className="text-muted">{t('search.filter.subnet')}</span>
        <select
          className={FIELD}
          data-testid="search-filter-subnet"
          value={filters.subnetId ?? ''}
          onChange={(event) => {
            setFilters({ subnetId: event.target.value === '' ? null : event.target.value });
          }}
        >
          <option value="">{t('search.filter.anySubnet')}</option>
          {subnetOptions.map((option) => (
            <option key={option.id} value={option.id}>
              {option.label}
            </option>
          ))}
        </select>
      </label>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-muted">{t('search.filter.from')}</span>
          <input
            type="date"
            className={FIELD}
            data-testid="search-filter-from"
            value={filters.from}
            onChange={(event) => {
              setFilters({ from: event.target.value });
            }}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-muted">{t('search.filter.to')}</span>
          <input
            type="date"
            className={FIELD}
            data-testid="search-filter-to"
            value={filters.to}
            onChange={(event) => {
              setFilters({ to: event.target.value });
            }}
          />
        </label>
      </div>
      <div className="flex flex-col gap-1">
        <label className={CHECK_LABEL}>
          <input
            type="checkbox"
            data-testid="search-filter-favorites"
            checked={filters.favoritesOnly}
            onChange={(event) => {
              setFilters({ favoritesOnly: event.target.checked });
            }}
          />
          {t('search.filter.favorites')}
        </label>
        <label className={CHECK_LABEL}>
          <input
            type="checkbox"
            data-testid="search-filter-archive"
            checked={filters.includeArchived}
            onChange={(event) => {
              setFilters({ includeArchived: event.target.checked });
            }}
          />
          {t('search.filter.archive')}
        </label>
        <label className={CHECK_LABEL}>
          <input
            type="checkbox"
            data-testid="search-filter-trash"
            checked={filters.includeTrash}
            onChange={(event) => {
              setFilters({ includeTrash: event.target.checked });
            }}
          />
          {t('search.filter.trash')}
        </label>
      </div>
      {filtering(filters) ? (
        <button
          type="button"
          className="self-start rounded-pw px-1.5 text-muted hover:text-fg"
          data-testid="search-filter-clear"
          onClick={clearFilters}
        >
          {t('search.filter.clear')}
        </button>
      ) : null}
    </div>
  );
}

export function SearchPanel() {
  const t = useT();
  const query = useSearch((state) => state.query);
  const status = useSearch((state) => state.status);
  const hits = useSearch((state) => state.hits);
  const error = useSearch((state) => state.error);
  const selected = useSearch((state) => state.selected);
  const filters = useSearch((state) => state.filters);
  const filtersVisible = useSearch((state) => state.filtersVisible);
  const focusRequest = useSearch((state) => state.focusRequest);
  const field = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, [focusRequest]);

  // The hits are on screen: for the measurement of the E2E test.
  useEffect(() => {
    if (status !== 'done') {
      return;
    }
    const frame = requestAnimationFrame(() => {
      performance.mark(SEARCH_SHOWN_MARK);
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [status, hits]);

  useEffect(() => {
    if (selected >= 0) {
      document.getElementById(optionId(selected))?.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);

  const store = useSearch.getState;
  const open = (index: number) => {
    const hit = hits[index];
    if (hit) {
      store().select(index);
      void openHit(hit);
    }
  };
  const onKeyDown = (event: KeyboardEvent) => {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        store().select(selected + 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        store().select(selected - 1);
        break;
      case 'PageDown':
        event.preventDefault();
        store().select(selected + 10);
        break;
      case 'PageUp':
        event.preventDefault();
        store().select(selected - 10);
        break;
      case 'Enter':
        if (selected >= 0) {
          event.preventDefault();
          open(selected);
        }
        break;
      default:
    }
  };

  const summary =
    status === 'error'
      ? error
        ? errorText(t, error)
        : t('search.failed')
      : status === 'done'
        ? hits.length === 0
          ? t('search.none')
          : t('search.count', { count: hits.length })
        : status === 'searching'
          ? t('search.searching')
          : '';

  return (
    <section className="flex h-full min-h-0 flex-col" aria-labelledby="search-title" data-testid="search">
      <header className="flex h-9 shrink-0 items-center justify-between px-3">
        <h2 id="search-title" className="text-ui font-semibold tracking-wide text-muted uppercase">
          {t('search.title')}
        </h2>
        <button
          type="button"
          className={`inline-flex size-7 items-center justify-center rounded-pw hover:bg-surface ${
            filtersVisible || filtering(filters) ? 'text-accent' : 'text-fg'
          }`}
          aria-label={t('search.filter.toggle')}
          aria-pressed={filtersVisible}
          title={t('search.filter.toggle')}
          data-testid="search-filter-toggle"
          onClick={() => {
            store().showFilters(!filtersVisible);
          }}
        >
          <Filter aria-hidden="true" className="size-4" />
        </button>
      </header>
      <div className="shrink-0 px-3 pb-2">
        <input
          ref={field}
          id={SEARCH_FIELD_ID}
          type="search"
          role="combobox"
          aria-expanded={hits.length > 0}
          aria-controls={LIST_ID}
          aria-activedescendant={selected >= 0 ? optionId(selected) : undefined}
          aria-label={t('search.field')}
          placeholder={t('search.field')}
          className={`${FIELD} w-full`}
          data-testid="search-field"
          value={query}
          onChange={(event) => {
            store().setQuery(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && query !== '') {
              event.stopPropagation();
              store().setQuery('');
              return;
            }
            onKeyDown(event);
          }}
        />
      </div>
      {filtersVisible ? <SearchFilters /> : null}
      <p
        role="status"
        className="text-ui shrink-0 px-3 pb-1 text-muted"
        data-testid="search-status"
        data-status={status}
      >
        {summary}
        {filtering(filters) && !filtersVisible ? ` · ${t('search.filter.active')}` : ''}
      </p>
      <ul
        ref={list}
        id={LIST_ID}
        role="listbox"
        tabIndex={hits.length > 0 ? 0 : -1}
        aria-label={t('search.results')}
        aria-activedescendant={selected >= 0 ? optionId(selected) : undefined}
        className="min-h-0 flex-1 overflow-y-auto pb-2 focus-visible:outline-offset-[-2px]"
        data-testid="search-results"
        onKeyDown={onKeyDown}
      >
        {hits.map((hit, index) => {
          const Icon = KIND_ICONS[hit.kind];
          return (
            <li
              key={`${hit.kind}:${hit.id}`}
              id={optionId(index)}
              role="option"
              aria-selected={index === selected}
              data-hit={hit.id}
              data-kind={hit.kind}
              data-match={hit.match}
              className="text-ui flex cursor-default gap-2 px-3 py-1 hover:bg-surface aria-selected:bg-surface aria-selected:shadow-[inset_2px_0_0_var(--color-accent)]"
              onMouseDown={(event) => {
                // Keep the focus where it is; the click opens the hit.
                event.preventDefault();
              }}
              onClick={() => {
                open(index);
              }}
            >
              <Icon aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-muted" />
              <span className="flex min-w-0 flex-col">
                <span className="flex items-center gap-1 text-fg">
                  <span className="truncate" data-title="">
                    {hit.title}
                  </span>
                  <span className="sr-only">{t(KIND_LABELS[hit.kind])}</span>
                  {hit.favorite ? (
                    <Star aria-label={t('search.badge.favorite')} className="size-3 shrink-0 text-muted" />
                  ) : null}
                  {hit.archived ? (
                    <Archive aria-label={t('search.badge.archived')} className="size-3 shrink-0 text-muted" />
                  ) : null}
                  {hit.deleted ? (
                    <Trash2 aria-label={t('search.badge.deleted')} className="size-3 shrink-0 text-muted" />
                  ) : null}
                </span>
                <Snippet hit={hit} />
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
