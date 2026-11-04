/**
 * Internal links (section 12; ADR 0015, decision 14): typing `[[` in rich
 * text opens this picker. The chosen document becomes a link to
 * `pwrite://<id>` with its title as text, so the link survives renaming.
 * In Markdown it is written as `[Titel](pwrite://<id>)`.
 */
import { useMemo, useState, type KeyboardEvent } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { useLocale, useT } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { getSession } from './session';

/** Most documents shown at once. */
const MAX_SHOWN = 50;

interface PickerStore {
  /** The tab that asked, or null while closed. */
  tab: string | null;
  open: (tab: string) => void;
  close: () => void;
}

export const useInternalLinkPicker = create<PickerStore>((set) => ({
  tab: null,
  open: (tab) => {
    set({ tab });
  },
  close: () => {
    set({ tab: null });
  },
}));

/** Documents whose title contains every word of the query, by title. */
export function matchingDocuments(
  documents: readonly DocumentSummary[],
  query: string,
  exclude: string,
  locale: string,
): DocumentSummary[] {
  const words = query.toLocaleLowerCase(locale).split(/\s+/).filter(Boolean);
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  return documents
    .filter((document) => document.id !== exclude)
    .filter((document) => {
      const title = document.title.toLocaleLowerCase(locale);
      return words.every((word) => title.includes(word));
    })
    .sort((a, b) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id))
    .slice(0, MAX_SHOWN);
}

export function InternalLinkPicker() {
  const tab = useInternalLinkPicker((state) => state.tab);
  return tab === null ? null : <Picker tab={tab} />;
}

function Picker({ tab }: { tab: string }) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const documents = useWorkspace((state) => state.documents);
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const shown = useMemo(
    () => matchingDocuments(documents, query, tab, locale),
    [documents, query, tab, locale],
  );
  const current = Math.min(index, shown.length - 1);

  const close = () => {
    useInternalLinkPicker.getState().close();
    getSession(tab)?.editor.commands.focus();
  };
  const choose = (document: DocumentSummary) => {
    useInternalLinkPicker.getState().close();
    getSession(tab)?.insertInternalLink(document.id, document.title);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      setIndex(Math.min(shown.length - 1, current + 1));
    } else if (event.key === 'ArrowUp') {
      setIndex(Math.max(0, current - 1));
    } else if (event.key === 'Enter') {
      const document = shown[current];
      if (document) {
        choose(document);
      }
    } else {
      return;
    }
    event.preventDefault();
  };

  const optionId = (id: string) => `pw-link-option-${id}`;
  const active = shown[current];
  return (
    <Dialog
      title={t('links.picker.title')}
      onClose={close}
      testId="internal-link-picker"
      width="w-[min(480px,92vw)]"
    >
      <input
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="pw-link-options"
        aria-activedescendant={active ? optionId(active.id) : undefined}
        aria-label={t('links.picker.search')}
        placeholder={t('links.picker.search')}
        className="text-ui h-8 w-full rounded-pw border border-muted bg-surface px-2 text-fg"
        value={query}
        autoFocus
        data-testid="internal-link-search"
        onChange={(event) => {
          setQuery(event.target.value);
          setIndex(0);
        }}
        onKeyDown={onKeyDown}
      />
      <ul
        id="pw-link-options"
        role="listbox"
        aria-label={t('links.picker.title')}
        className="mt-2 max-h-72 overflow-y-auto"
      >
        {shown.map((document, position) => (
          <li
            key={document.id}
            id={optionId(document.id)}
            role="option"
            aria-selected={position === current}
            data-document={document.id}
            className={`text-ui cursor-default truncate rounded-pw px-2 py-1 ${
              position === current ? 'bg-surface text-fg' : 'text-fg'
            }`}
            onMouseDown={(event) => {
              event.preventDefault();
              choose(document);
            }}
          >
            {document.title}
          </li>
        ))}
      </ul>
      {shown.length === 0 ? (
        <p className="text-ui mt-2 text-muted" role="status">
          {t('links.picker.empty')}
        </p>
      ) : null}
    </Dialog>
  );
}
