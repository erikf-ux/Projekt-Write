/**
 * The tags of one document (section 13): the tags it has, each with a
 * button to take it away, and a field that suggests existing tags and
 * makes a new one from a new name. Tags are a filter, not a hierarchy.
 */
import { X } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useLocale, useT } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import type { CommandError } from '@/types/generated/CommandError';
import { useStructureDialogs } from './dialogStore';
import { NO_PLACEMENT, useStructure } from './structureStore';

const SUGGESTIONS = 8;

export function TagsDialog() {
  const documentId = useStructureDialogs((store) => store.tags);
  return documentId ? <TagsForm key={documentId} documentId={documentId} /> : null;
}

function TagsForm({ documentId }: { documentId: string }) {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const tags = useStructure((store) => store.tags);
  const placement = useStructure((store) => store.byElement[documentId] ?? NO_PLACEMENT);
  const title = useWorkspace(
    (store) => store.documents.find((document) => document.id === documentId)?.title ?? '',
  );
  const [name, setName] = useState('');
  const [error, setError] = useState<CommandError | null>(null);
  const close = () => {
    useStructureDialogs.getState().close();
  };
  const own = placement.tagIds
    .map((id) => tags.find((tag) => tag.id === id))
    .filter((tag): tag is NonNullable<typeof tag> => tag !== undefined);
  const suggestions = useMemo(() => {
    const query = name.trim().toLocaleLowerCase(locale);
    if (query === '') {
      return [];
    }
    return tags
      .filter((tag) => !placement.tagIds.includes(tag.id))
      .filter((tag) => tag.name.toLocaleLowerCase(locale).includes(query))
      .slice(0, SUGGESTIONS);
  }, [tags, name, locale, placement.tagIds]);

  const add = async (value: string) => {
    if (value.trim() === '') {
      return;
    }
    const result = await useStructure.getState().tagDocument(documentId, value);
    setError(result);
    if (!result) {
      setName('');
    }
  };

  return (
    <Dialog
      title={t('tags.title', { title })}
      onClose={close}
      testId="tags-dialog"
      width="w-[min(480px,92vw)]"
    >
      <div className="text-ui flex flex-col gap-3">
        <ul className="flex flex-wrap gap-1.5" aria-label={t('tags.current')} data-testid="tags-current">
          {own.length === 0 ? <li className="text-muted">{t('tags.none')}</li> : null}
          {own.map((tag) => (
            <li key={tag.id} data-tag={tag.id}>
              <span className="inline-flex h-6 items-center gap-1 rounded-pw border border-muted px-1.5">
                # {tag.name}
                <button
                  type="button"
                  aria-label={t('tags.remove', { tag: tag.name })}
                  className="inline-flex size-4 items-center justify-center rounded-pw text-muted hover:text-fg"
                  onClick={() => void useStructure.getState().untagDocument(documentId, tag.id)}
                >
                  <X aria-hidden="true" className="size-3" />
                </button>
              </span>
            </li>
          ))}
        </ul>
        <form
          className="flex flex-col gap-1"
          onSubmit={(event) => {
            event.preventDefault();
            void add(name);
          }}
        >
          <label className="flex flex-col gap-1">
            {t('tags.add')}
            <input
              type="text"
              data-autofocus
              data-testid="tags-input"
              maxLength={100}
              className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
              value={name}
              onChange={(event) => {
                setName(event.target.value);
              }}
            />
          </label>
          <p className="text-muted">{t('tags.hint')}</p>
          {suggestions.length > 0 ? (
            <ul className="flex flex-wrap gap-1" aria-label={t('tags.suggestions')}>
              {suggestions.map((tag) => (
                <li key={tag.id}>
                  <button
                    type="button"
                    data-tag-suggestion={tag.id}
                    className="inline-flex h-6 items-center rounded-pw border border-muted px-1.5 hover:bg-surface"
                    onClick={() => void add(tag.name)}
                  >
                    # {tag.name}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
          {error ? (
            <p role="alert" className="text-danger">
              {errorText(t, error)}
            </p>
          ) : null}
          <div className="flex justify-end gap-2 pt-1">
            <button type="button" className="text-ui h-7 rounded-pw border border-muted px-3" onClick={close}>
              {t('tags.close')}
            </button>
            <button
              type="submit"
              data-testid="tags-submit"
              className="text-ui h-7 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-50"
              disabled={name.trim() === ''}
            >
              {t('tags.addButton')}
            </button>
          </div>
        </form>
      </div>
    </Dialog>
  );
}
