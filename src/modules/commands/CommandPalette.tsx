/**
 * Command palette (section 11, `Ctrl + Shift + P`): lists the commands that
 * work right now, filtered by what the user types.
 *
 * ARIA combobox pattern: the input keeps the focus, arrow keys move the
 * active option (`aria-activedescendant`), Enter runs it, Escape closes.
 */
import { Search } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useT } from '@/i18n';
import type { CommandId } from './types';

export interface PaletteEntry {
  id: CommandId;
  title: string;
  category: string;
  /** Shortcut as shown to the user. */
  shortcut?: string;
}

/** Entries whose title or category contains every word of the query. */
export function filterEntries(entries: readonly PaletteEntry[], query: string): PaletteEntry[] {
  const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    return [...entries];
  }
  return entries
    .map((entry) => {
      const title = entry.title.toLocaleLowerCase();
      const text = `${title} ${entry.category.toLocaleLowerCase()}`;
      if (!words.every((word) => text.includes(word))) {
        return null;
      }
      // Titles starting with the query come first.
      const rank = title.startsWith(words[0] ?? '') ? 0 : 1;
      return { entry, rank };
    })
    .filter((match): match is { entry: PaletteEntry; rank: number } => match !== null)
    .sort((a, b) => a.rank - b.rank)
    .map((match) => match.entry);
}

interface CommandPaletteProps {
  entries: readonly PaletteEntry[];
  onRun: (id: CommandId) => void;
  onClose: () => void;
}

export function CommandPalette({ entries, onRun, onClose }: CommandPaletteProps) {
  const t = useT();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();
  const titleId = useId();
  const matches = useMemo(() => filterEntries(entries, query), [entries, query]);
  const current = matches[Math.min(active, matches.length - 1)];

  useEffect(() => {
    input.current?.focus();
  }, []);

  useEffect(() => {
    if (current) {
      document.getElementById(`${listId}-${current.id}`)?.scrollIntoView({ block: 'nearest' });
    }
  }, [current, listId]);

  const run = (entry: PaletteEntry | undefined) => {
    if (entry) {
      onClose();
      onRun(entry.id);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const last = matches.length - 1;
    switch (event.key) {
      case 'ArrowDown':
        setActive((index) => (index >= last ? 0 : index + 1));
        break;
      case 'ArrowUp':
        setActive((index) => (index <= 0 ? last : index - 1));
        break;
      case 'PageDown':
        setActive((index) => Math.min(last, index + 8));
        break;
      case 'PageUp':
        setActive((index) => Math.max(0, index - 8));
        break;
      case 'Enter':
        run(current);
        break;
      case 'Escape':
        onClose();
        break;
      case 'Tab':
        // The palette is modal: the focus stays in the input.
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  };

  return (
    <div className="fixed inset-0 z-40 flex justify-center bg-black/30 pt-[12vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-testid="command-palette"
        className="h-fit w-[min(560px,90vw)] overflow-hidden rounded-pw border border-muted bg-sidebar text-fg shadow-lg"
        onMouseDown={(event) => {
          event.stopPropagation();
        }}
      >
        <h2 id={titleId} className="sr-only">
          {t('palette.title')}
        </h2>
        <div className="flex items-center gap-2 border-b border-muted px-3">
          <Search aria-hidden="true" className="size-4 shrink-0 text-muted" />
          <input
            ref={input}
            role="combobox"
            aria-expanded="true"
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={current ? `${listId}-${current.id}` : undefined}
            aria-label={t('palette.title')}
            placeholder={t('palette.placeholder')}
            className="text-ui h-9 w-full bg-transparent text-fg outline-none placeholder:text-muted"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
            onKeyDown={onKeyDown}
          />
        </div>
        <ul
          id={listId}
          role="listbox"
          aria-label={t('palette.title')}
          className="max-h-[50vh] overflow-y-auto py-1"
        >
          {matches.map((entry) => {
            const selected = entry.id === current?.id;
            return (
              <li
                key={entry.id}
                id={`${listId}-${entry.id}`}
                role="option"
                aria-selected={selected}
                data-command={entry.id}
                className={`text-ui flex cursor-default items-center justify-between gap-4 px-3 py-1.5 ${
                  selected ? 'bg-accent text-surface' : 'text-fg'
                }`}
                onMouseMove={() => {
                  setActive(matches.indexOf(entry));
                }}
                onClick={() => {
                  run(entry);
                }}
              >
                <span>
                  <span className={selected ? 'text-surface' : 'text-muted'}>{entry.category}: </span>
                  {entry.title}
                </span>
                {entry.shortcut ? (
                  <kbd className={`font-sans ${selected ? 'text-surface' : 'text-muted'}`}>
                    {entry.shortcut}
                  </kbd>
                ) : null}
              </li>
            );
          })}
          {matches.length === 0 ? (
            <li role="presentation" className="text-ui px-3 py-2 text-muted">
              {t('palette.empty')}
            </li>
          ) : null}
        </ul>
      </div>
    </div>
  );
}
