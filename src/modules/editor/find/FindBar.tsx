/**
 * The find bar of a tab (section 12: "Suchen und Ersetzen im Dokument mit
 * Treffernavigation, Groß-/Kleinschreibung, ganzen Wörtern und regulären
 * Ausdrücken"). Regular expressions use RE2 syntax: no backtracking, so no
 * search can hang the editor.
 *
 * Enter: next result, Shift+Enter: previous, Escape: close (the focus goes
 * back to the text).
 */
import { createSearchRegex } from '@tiptap/extension-find-and-replace';
import { ArrowDown, ArrowUp, CaseSensitive, Regex, WholeWord, X } from 'lucide-react';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useT } from '@/i18n';
import type { DocumentView } from '@/modules/documents/workspaceStore';
import { getSession, type SearchResults } from '../session';
import { useFind, type FindState } from './findStore';

interface FindBarProps {
  id: string;
  state: FindState;
  /** The current view; a switch runs the search in the other view. */
  view: DocumentView;
  readOnly: boolean;
}

const FIELD =
  'text-ui h-7 min-w-0 flex-1 rounded-pw border border-muted bg-surface px-2 text-fg aria-[invalid=true]:border-danger';
const ICON_BUTTON =
  'inline-flex size-7 shrink-0 items-center justify-center rounded-pw text-muted hover:bg-surface hover:text-fg aria-pressed:bg-surface aria-pressed:text-fg aria-pressed:shadow-[inset_0_0_0_1px_var(--color-accent)] disabled:opacity-50';
const TEXT_BUTTON =
  'text-ui inline-flex h-7 shrink-0 items-center rounded-pw border border-muted px-2 disabled:opacity-50';

export function FindBar({ id, state, view, readOnly }: FindBarProps) {
  const t = useT();
  const field = useRef<HTMLInputElement>(null);
  const [results, setResults] = useState<SearchResults>({ count: 0, current: null });
  const { term, replacement, caseSensitive, wholeWord, regex } = state;
  const invalid =
    regex &&
    term !== '' &&
    createSearchRegex(term, { caseSensitive, useRegex: true, wholeWord: false }) === null;

  // Run the search in the editor and follow its results while the text changes.
  useEffect(() => {
    const session = getSession(id);
    if (!session) {
      return;
    }
    const editor = session.editor;
    session.search({ term, replacement, caseSensitive, wholeWord, regex });
    const update = () => {
      setResults(session.searchResults());
    };
    update();
    editor.on('transaction', update);
    return () => {
      editor.off('transaction', update);
    };
  }, [id, view, term, replacement, caseSensitive, wholeWord, regex]);

  // Focus the search field when the bar opens or is asked for again (Ctrl+F).
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, [state.focusRequest]);

  const change = (update: Partial<FindState>) => {
    useFind.getState().change(id, update);
  };
  const close = () => {
    getSession(id)?.clearSearch();
    useFind.getState().close(id);
    getSession(id)?.focus();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>, onEnter: () => void) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (event.shiftKey) {
        getSession(id)?.findPrevious();
      } else {
        onEnter();
      }
    }
  };
  const count =
    term === ''
      ? ''
      : invalid
        ? t('find.invalid')
        : results.count === 0
          ? t('find.none')
          : t('find.count', { current: (results.current ?? -1) + 1 || '–', count: results.count });
  const none = results.count === 0;

  return (
    <div
      role="search"
      aria-label={t('find.label')}
      data-testid="find-bar"
      className="flex shrink-0 flex-col gap-1 border-b border-muted/40 bg-sidebar px-4 py-1.5"
    >
      <div className="flex items-center gap-1">
        <input
          ref={field}
          type="search"
          className={FIELD}
          value={term}
          placeholder={t('find.placeholder')}
          aria-label={t('find.term')}
          aria-invalid={invalid}
          data-testid="find-term"
          onChange={(event) => {
            change({ term: event.target.value });
          }}
          onKeyDown={(event) => {
            onKeyDown(event, () => getSession(id)?.findNext());
          }}
        />
        <span
          className="text-ui w-24 shrink-0 text-center text-muted tabular-nums"
          aria-live="polite"
          data-testid="find-count"
        >
          {count}
        </span>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-pressed={caseSensitive}
          aria-label={t('find.caseSensitive')}
          title={t('find.caseSensitive')}
          data-testid="find-case"
          onClick={() => {
            change({ caseSensitive: !caseSensitive });
          }}
        >
          <CaseSensitive aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-pressed={wholeWord}
          aria-label={t('find.wholeWord')}
          title={t('find.wholeWord')}
          disabled={regex}
          data-testid="find-whole-word"
          onClick={() => {
            change({ wholeWord: !wholeWord });
          }}
        >
          <WholeWord aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-pressed={regex}
          aria-label={t('find.regex')}
          title={t('find.regex')}
          data-testid="find-regex"
          onClick={() => {
            change({ regex: !regex });
          }}
        >
          <Regex aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-label={t('find.previous')}
          title={t('find.previous')}
          disabled={none}
          data-testid="find-previous"
          onClick={() => getSession(id)?.findPrevious()}
        >
          <ArrowUp aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-label={t('find.next')}
          title={t('find.next')}
          disabled={none}
          data-testid="find-next"
          onClick={() => getSession(id)?.findNext()}
        >
          <ArrowDown aria-hidden="true" className="size-4" />
        </button>
        <button
          type="button"
          className={ICON_BUTTON}
          aria-label={t('find.close')}
          title={t('find.close')}
          data-testid="find-close"
          onClick={close}
        >
          <X aria-hidden="true" className="size-4" />
        </button>
      </div>
      {state.replace ? (
        <div className="flex items-center gap-1">
          <input
            type="text"
            className={FIELD}
            value={replacement}
            placeholder={t('find.replacement')}
            aria-label={t('find.replacement')}
            data-testid="find-replacement"
            onChange={(event) => {
              change({ replacement: event.target.value });
            }}
            onKeyDown={(event) => {
              onKeyDown(event, () => getSession(id)?.replaceCurrent());
            }}
          />
          <button
            type="button"
            className={TEXT_BUTTON}
            disabled={none || readOnly}
            data-testid="find-replace"
            onClick={() => getSession(id)?.replaceCurrent()}
          >
            {t('find.replace')}
          </button>
          <button
            type="button"
            className={TEXT_BUTTON}
            disabled={none || readOnly}
            data-testid="find-replace-all"
            onClick={() => getSession(id)?.replaceAll()}
          >
            {t('find.replaceAll')}
          </button>
        </div>
      ) : null}
    </div>
  );
}
