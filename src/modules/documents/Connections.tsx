/**
 * "Verbindungen" (section 12; ADR 0015, decision 14): the documents that
 * link to the active library document. Rust keeps the links up to date on
 * every save; the list is asked for when the active document changes or
 * any document is saved.
 */
import { Link2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useT } from '@/i18n';
import { call } from '@/services/ipc';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { useWorkspace } from './workspaceStore';

export function Connections() {
  const t = useT();
  const active = useWorkspace((state) => state.active);
  const activeDocument = useWorkspace((state) => (state.active ? state.open[state.active] : undefined));
  const documents = useWorkspace((state) => state.documents);
  const openDocument = useWorkspace((state) => state.openDocument);
  const [backlinks, setBacklinks] = useState<{ id: string; list: DocumentSummary[] } | null>(null);
  const libraryId = activeDocument && !activeDocument.external ? active : null;

  useEffect(() => {
    if (libraryId === null) {
      return;
    }
    let current = true;
    void call('document_backlinks', { id: libraryId }).then((result) => {
      if (current && result.ok) {
        setBacklinks({ id: libraryId, list: result.value });
      }
    });
    return () => {
      current = false;
    };
    // `documents` changes with every save: links may have changed.
  }, [libraryId, documents]);

  if (libraryId === null) {
    return null;
  }
  const list = backlinks?.id === libraryId ? backlinks.list : [];
  return (
    <section
      aria-labelledby="connections-title"
      className="flex max-h-[40%] shrink-0 flex-col border-t border-muted/40"
      data-testid="connections"
    >
      <h2
        id="connections-title"
        className="text-ui flex h-8 shrink-0 items-center gap-1.5 px-3 font-semibold tracking-wide text-muted uppercase"
      >
        <Link2 aria-hidden="true" className="size-3.5" />
        {t('connections.title')}
      </h2>
      <p className="text-ui px-3 pb-1 text-muted">{t('connections.backlinks')}</p>
      {list.length === 0 ? (
        <p className="text-ui px-3 pb-2 text-muted" data-testid="connections-empty">
          {t('connections.empty')}
        </p>
      ) : (
        <ul className="min-h-0 overflow-y-auto pb-2">
          {list.map((document) => (
            <li key={document.id}>
              <button
                type="button"
                data-backlink={document.id}
                className="text-ui w-full truncate px-3 py-0.5 text-left text-fg hover:bg-surface"
                onClick={() => void openDocument(document.id)}
              >
                {document.title}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
