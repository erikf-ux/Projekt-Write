/**
 * The writing surface of Phase 3: plain text for `.txt` and the Markdown
 * source of `.md` (ADR 0011). Phase 4 replaces it with the Tiptap editor.
 *
 * Every open tab keeps its own text field, so its undo history lasts as
 * long as the tab is open (section 12). Hidden fields stay mounted.
 */
import { FilePlus } from 'lucide-react';
import { useT } from '@/i18n';
import type { OpenDocument } from '@/modules/documents/workspaceStore';

export function editorElementId(id: string): string {
  return `pw-editor-${id}`;
}

export function panelElementId(id: string): string {
  return `pw-panel-${id}`;
}

interface PlainTextEditorProps {
  tabs: readonly string[];
  open: Readonly<Record<string, OpenDocument>>;
  active: string | null;
  tabElementId: (id: string) => string;
  limitLineWidth: boolean;
  focusMode: boolean;
  /** While the library moves, nothing can be saved. */
  readOnly: boolean;
  onEdit: (id: string, text: string) => void;
  onLeave: (id: string) => void;
  onNewDocument: (() => void) | null;
  newDocumentShortcut?: string | undefined;
}

export function PlainTextEditor({
  tabs,
  open,
  active,
  tabElementId,
  limitLineWidth,
  focusMode,
  readOnly,
  onEdit,
  onLeave,
  onNewDocument,
  newDocumentShortcut,
}: PlainTextEditorProps) {
  const t = useT();

  if (tabs.length === 0) {
    return (
      <div
        className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 text-muted"
        data-testid="editor-empty"
      >
        <p className="text-ui">{t('editor.empty')}</p>
        {onNewDocument ? (
          <button
            type="button"
            className="text-ui inline-flex h-7 items-center gap-2 rounded-pw bg-accent px-3 font-medium text-surface"
            onClick={onNewDocument}
          >
            <FilePlus aria-hidden="true" className="size-3.5" />
            {t('command.document.new')}
            {newDocumentShortcut ? <kbd className="font-sans">{newDocumentShortcut}</kbd> : null}
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <div className="relative min-h-0 flex-1">
      {tabs.map((id) => {
        const document = open[id];
        if (!document) {
          return null;
        }
        return (
          <div
            key={id}
            id={panelElementId(id)}
            role="tabpanel"
            aria-labelledby={tabElementId(id)}
            hidden={id !== active}
            className="absolute inset-0 bg-surface"
          >
            {document.loaded ? (
              <textarea
                id={editorElementId(id)}
                data-testid="editor"
                aria-label={t('editor.label', { title: document.title })}
                className={`text-body block h-full w-full resize-none bg-surface text-fg outline-none focus:shadow-[inset_2px_0_0_0_var(--color-accent)] ${
                  focusMode ? 'py-12' : 'py-6'
                } ${
                  // The field fills the panel (focus bar and scroll bar at the
                  // edges); the padding keeps lines at most 72 characters.
                  limitLineWidth ? 'px-[max(1.5rem,calc((100%-72ch)/2))]' : focusMode ? 'px-8' : 'px-6'
                }`}
                value={document.text}
                readOnly={readOnly}
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                onChange={(event) => {
                  onEdit(id, event.target.value);
                }}
                onBlur={() => {
                  onLeave(id);
                }}
              />
            ) : (
              <p role="status" className="text-ui p-6 text-muted">
                {t('editor.loading')}
              </p>
            )}
          </div>
        );
      })}
    </div>
  );
}
