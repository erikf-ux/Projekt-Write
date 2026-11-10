/**
 * "Tags verwalten" (section 13): every tag with the number of documents
 * that have it; rename (onto the name of another tag: merge, after a
 * question), merge into another tag, delete (documents stay). Searchable;
 * a library has a thousand tags or more.
 */
import { useMemo, useState } from 'react';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useLocale, useT } from '@/i18n';
import type { CommandError } from '@/types/generated/CommandError';
import { useStructureDialogs } from './dialogStore';
import { useStructure } from './structureStore';

const SHOWN = 200;

export function TagManager() {
  const open = useStructureDialogs((store) => store.tagManager);
  return open ? <Manager /> : null;
}

function Manager() {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const tags = useStructure((store) => store.tags);
  const byElement = useStructure((store) => store.byElement);
  const [query, setQuery] = useState('');
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const [merging, setMerging] = useState<string | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const close = () => {
    useStructureDialogs.getState().close();
  };

  const counts = useMemo(() => {
    const result = new Map<string, number>();
    for (const placement of Object.values(byElement)) {
      for (const id of placement.tagIds) {
        result.set(id, (result.get(id) ?? 0) + 1);
      }
    }
    return result;
  }, [byElement]);
  const shown = useMemo(() => {
    const words = query.toLocaleLowerCase(locale).split(/\s+/).filter(Boolean);
    return tags
      .filter((tag) => words.every((word) => tag.name.toLocaleLowerCase(locale).includes(word)))
      .slice(0, SHOWN);
  }, [tags, query, locale]);

  const rename = async () => {
    if (!editing) {
      return;
    }
    const result = await useStructure.getState().renameTag(editing.id, editing.name);
    setError(result);
    if (!result) {
      setEditing(null);
    }
  };

  return (
    <Dialog title={t('tags.manageTitle')} onClose={close} testId="tag-manager" width="w-[min(620px,94vw)]">
      <div className="text-ui flex flex-col gap-2">
        <input
          type="search"
          data-autofocus
          data-testid="tag-manager-search"
          aria-label={t('tags.search')}
          placeholder={t('tags.search')}
          className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
        <p className="text-muted" role="status">
          {t('tags.count', { count: tags.length, shown: shown.length })}
        </p>
        <ul className="flex max-h-[50vh] flex-col overflow-y-auto" aria-label={t('tags.manageTitle')}>
          {shown.map((tag) => (
            <li
              key={tag.id}
              data-managed-tag={tag.id}
              className="flex items-center gap-2 border-b border-muted/30 py-1"
            >
              {editing?.id === tag.id ? (
                <form
                  className="flex flex-1 items-center gap-2"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void rename();
                  }}
                >
                  <input
                    type="text"
                    autoFocus
                    data-testid="tag-rename"
                    aria-label={t('tags.renameLabel', { tag: tag.name })}
                    maxLength={100}
                    className="text-ui h-7 flex-1 rounded-pw border border-accent bg-surface px-2 text-fg"
                    value={editing.name}
                    onChange={(event) => {
                      setEditing({ id: tag.id, name: event.target.value });
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Escape') {
                        event.stopPropagation();
                        setEditing(null);
                      }
                    }}
                  />
                  <button type="submit" className="text-ui h-7 rounded-pw bg-accent px-2 text-surface">
                    {t('tags.save')}
                  </button>
                </form>
              ) : merging === tag.id ? (
                <label className="flex flex-1 items-center gap-2">
                  {t('tags.mergeInto', { tag: tag.name })}
                  <select
                    data-testid="tag-merge-target"
                    className="text-ui h-7 flex-1 rounded-pw border border-muted bg-surface px-1 text-fg"
                    defaultValue=""
                    onChange={(event) => {
                      const target = event.target.value;
                      if (target) {
                        setMerging(null);
                        void useStructure
                          .getState()
                          .mergeTags(tag.id, target)
                          .then((result) => {
                            setError(result);
                          });
                      }
                    }}
                  >
                    <option value="">{t('tags.mergeChoose')}</option>
                    {tags
                      .filter((other) => other.id !== tag.id)
                      .map((other) => (
                        <option key={other.id} value={other.id}>
                          {other.name}
                        </option>
                      ))}
                  </select>
                </label>
              ) : (
                <>
                  <span className="flex-1 truncate"># {tag.name}</span>
                  <span className="text-muted">
                    {t('tags.documents', { count: counts.get(tag.id) ?? 0 })}
                  </span>
                  <button
                    type="button"
                    className="text-ui h-6 rounded-pw border border-muted px-2 hover:bg-surface"
                    data-action="rename"
                    onClick={() => {
                      setEditing({ id: tag.id, name: tag.name });
                    }}
                  >
                    {t('tags.rename')}
                  </button>
                  <button
                    type="button"
                    className="text-ui h-6 rounded-pw border border-muted px-2 hover:bg-surface"
                    data-action="merge"
                    disabled={tags.length < 2}
                    onClick={() => {
                      setMerging(tag.id);
                    }}
                  >
                    {t('tags.mergeButton')}
                  </button>
                  <button
                    type="button"
                    className="text-ui h-6 rounded-pw border border-danger px-2 text-danger hover:bg-surface"
                    data-action="delete"
                    onClick={() =>
                      void useStructure
                        .getState()
                        .deleteTag(tag.id)
                        .then((result) => {
                          setError(result);
                        })
                    }
                  >
                    {t('tags.deleteButton')}
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
        {tags.length === 0 ? <p className="text-muted">{t('tags.noneInLibrary')}</p> : null}
        {error ? (
          <p role="alert" className="text-danger">
            {errorText(t, error)}
          </p>
        ) : null}
        <div className="flex justify-end">
          <button type="button" className="text-ui h-7 rounded-pw border border-muted px-3" onClick={close}>
            {t('tags.close')}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
