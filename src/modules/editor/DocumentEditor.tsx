/**
 * The writing surface of Phase 4 (section 12): one editor session per tab
 * (`session.ts`), rich text for `.md` with a source view, plain text for
 * `.txt`. Hidden tabs stay mounted, so switching is instant and every tab
 * keeps its undo history while it is open.
 *
 * Above the text: why a Markdown document opened in the source view
 * (round-trip probe), and what saving as Markdown would lose (loss check,
 * section 10), with "Trotzdem speichern" and "Abbrechen". External files
 * add notes for a postponed change from outside, a read-only file, mixed
 * line endings and failed saves with the way out ("Speichern unter",
 * "Als UTF-8 speichern").
 */
import { FilePlus, X } from 'lucide-react';
import { useEffect, useEffectEvent, useRef } from 'react';
import { errorText } from '@/components/errorText';
import { translate, useLocale, useT, type MessageKey } from '@/i18n';
import { useWorkspace, setDocumentSource, type OpenDocument } from '@/modules/documents/workspaceStore';
import { LOSS_KEYS } from './markdown/lossKeys';
import type { Loss } from './markdown/losses';
import type { ProbeIssue } from './markdown/probe';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import { FindBar } from './find/FindBar';
import { useFind } from './find/findStore';
import { useInternalLinkPicker } from './InternalLinkPicker';
import { internalLinkTarget } from './schema/extensions';
import { getSession, openSession } from './session';
import './editor.css';

/**
 * Opens a link: an internal one (`pwrite://<id>`) as a tab of the app, a web
 * link in the system browser (only http, https and mailto; Rust checks).
 */
async function openLink(href: string): Promise<void> {
  const target = internalLinkTarget(href);
  if (target !== null) {
    const workspace = useWorkspace.getState();
    if (!workspace.documents.some((document) => document.id === target)) {
      useUi.getState().showNotice({ tone: 'error', key: 'links.missing' });
      return;
    }
    await workspace.openDocument(target);
    return;
  }
  const result = await call('link_open', { url: href });
  if (!result.ok) {
    useUi.getState().showNotice({ tone: 'error', key: 'error.linkOpenFailed', error: result.error });
  }
}

export function panelElementId(id: string): string {
  return `pw-panel-${id}`;
}

interface DocumentEditorProps {
  tabs: readonly string[];
  open: Readonly<Record<string, OpenDocument>>;
  active: string | null;
  tabElementId: (id: string) => string;
  limitLineWidth: boolean;
  focusMode: boolean;
  /** While the library moves, nothing can be saved. */
  readOnly: boolean;
  onNewDocument: (() => void) | null;
  newDocumentShortcut?: string | undefined;
}

export function DocumentEditor({
  tabs,
  open,
  active,
  tabElementId,
  limitLineWidth,
  focusMode,
  readOnly,
  onNewDocument,
  newDocumentShortcut,
}: DocumentEditorProps) {
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
        return document ? (
          <DocumentPanel
            key={id}
            document={document}
            hidden={id !== active}
            labelledBy={tabElementId(id)}
            limitLineWidth={limitLineWidth}
            focusMode={focusMode}
            readOnly={readOnly}
          />
        ) : null;
      })}
    </div>
  );
}

interface DocumentPanelProps {
  document: OpenDocument;
  hidden: boolean;
  labelledBy: string;
  limitLineWidth: boolean;
  focusMode: boolean;
  readOnly: boolean;
}

function DocumentPanel({
  document,
  hidden,
  labelledBy,
  limitLineWidth,
  focusMode,
  readOnly: libraryReadOnly,
}: DocumentPanelProps) {
  const t = useT();
  const host = useRef<HTMLDivElement>(null);
  const { id, loaded, generation } = document;
  // The library is not writable, or the file is of a newer format version.
  const readOnly = libraryReadOnly || document.readOnly;
  const find = useFind((state) => state.bars[document.id]);
  const label = t('editor.label', { title: document.title });

  // The session is created once per tab, from the values of that moment;
  // later changes reach it through the effects below.
  const attach = useEffectEvent((element: HTMLDivElement) => {
    const session = openSession({
      id,
      format: document.format,
      text: document.loadedText,
      label,
      editable: !readOnly,
      onChange: () => {
        useWorkspace.getState().edit(id);
      },
      onBlur: () => {
        void useWorkspace.getState().flush(id);
      },
      onOpenLink: (href) => {
        void openLink(href);
      },
      onInternalLinkRequest: () => {
        useInternalLinkPicker.getState().open(id);
      },
    });
    setDocumentSource(id, session);
    element.append(session.root);
    useWorkspace
      .getState()
      .attachView(
        id,
        session.view === 'rich' ? 'rich' : session.isMarkdown ? 'source' : 'plain',
        session.openIssues,
      );
    return session;
  });

  useEffect(() => {
    const element = host.current;
    if (!loaded || !element) {
      return;
    }
    const session = attach(element);
    return () => {
      session.root.remove();
    };
  }, [id, loaded, generation]);

  useEffect(() => {
    getSession(id)?.setEditable(!readOnly);
  }, [id, readOnly]);

  useEffect(() => {
    getSession(id)?.setLabel(label);
  }, [id, label]);

  return (
    <div
      id={panelElementId(id)}
      role="tabpanel"
      aria-labelledby={labelledBy}
      hidden={hidden}
      className="absolute inset-0 flex flex-col bg-surface"
    >
      {document.readOnly ? (
        <p
          role="status"
          className="text-ui border-b border-muted/40 bg-sidebar px-4 py-2 text-fg"
          data-testid="read-only-note"
        >
          {t('editor.readOnlyNewer')}
        </p>
      ) : null}
      {document.external ? <ExternalNotes document={document} /> : null}
      {document.viewIssues.length > 0 ? (
        <ViewIssues
          issues={document.viewIssues}
          opened={document.view === 'source'}
          onClose={() => {
            useWorkspace.getState().dismissViewIssues(id);
          }}
        />
      ) : null}
      {document.state === 'blocked' && document.losses.length > 0 ? (
        <LossWarning
          losses={document.losses}
          onSaveAsPwrite={
            document.external || document.format !== 'md'
              ? null
              : () => void useWorkspace.getState().convertDocument(id, 'pwrite')
          }
          onSaveAnyway={() => void useWorkspace.getState().saveAnyway(id)}
          onCancel={() => {
            useWorkspace.getState().dismissLosses(id);
          }}
        />
      ) : null}
      {find && loaded ? <FindBar id={id} state={find} view={document.view} readOnly={readOnly} /> : null}
      {loaded ? (
        <div
          ref={host}
          className="min-h-0 flex-1 overflow-auto"
          data-limit-width={limitLineWidth}
          data-focus-mode={focusMode}
        />
      ) : (
        <p role="status" className="text-ui p-6 text-muted">
          {t('editor.loading')}
        </p>
      )}
    </div>
  );
}

const NOTE = 'text-ui flex shrink-0 items-center gap-3 border-b border-muted/40 bg-sidebar px-4 py-2';
const NOTE_BUTTON = 'inline-flex h-6 shrink-0 items-center rounded-pw border border-muted px-2';

/** Notes of an external file above its text. */
function ExternalNotes({ document }: { document: OpenDocument }) {
  const t = useT();
  const external = document.external;
  if (!external) {
    return null;
  }
  const { id } = document;
  const workspace = useWorkspace.getState;
  const saveAs = () => {
    void workspace().saveAs(id, translate(useLocale.getState().locale, 'external.dialog.saveAs'));
  };
  const error = document.state === 'error' ? document.error : null;
  return (
    <>
      {external.outside && external.outsideDeferred ? (
        <div role="status" className={NOTE} data-testid="outside-note">
          <p className="min-w-0 flex-1">
            {t(external.outside === 'changed' ? 'external.changed.title' : 'external.missing.title')}
          </p>
          <button
            type="button"
            className={NOTE_BUTTON}
            onClick={() => {
              workspace().deferOutside(id, false);
            }}
          >
            {t('external.decide')}
          </button>
        </div>
      ) : null}
      {error ? (
        <div role="alert" className={`${NOTE} border-b-danger`} data-testid="external-save-error">
          <p className="min-w-0 flex-1">{errorText(t, error)}</p>
          {error.code === 'FILE_UNENCODABLE' ? (
            <button
              type="button"
              className={NOTE_BUTTON}
              onClick={() => {
                workspace().setEncoding(id, 'utf8');
                void workspace().save(id);
              }}
            >
              {t('external.saveUtf8')}
            </button>
          ) : null}
          {error.code === 'FILE_READ_ONLY' || error.code === 'FILE_MISSING' ? (
            <button type="button" className={NOTE_BUTTON} onClick={saveAs}>
              {t('external.saveAs')}
            </button>
          ) : null}
        </div>
      ) : external.fileReadOnly && !document.readOnly ? (
        <div role="status" className={NOTE} data-testid="file-read-only-note">
          <p className="min-w-0 flex-1">{t('external.readOnlyFile')}</p>
          <button type="button" className={NOTE_BUTTON} onClick={saveAs}>
            {t('external.saveAs')}
          </button>
        </div>
      ) : null}
      {external.mixedLineEndings ? (
        <p role="status" className={NOTE} data-testid="mixed-line-endings">
          {t('external.mixedLineEndings', { lineEnding: t(`lineEnding.${external.lineEnding}`) })}
        </p>
      ) : null}
    </>
  );
}

const BLOCK_KEYS: Record<string, MessageKey> = {
  paragraph: 'editor.block.paragraph',
  heading: 'editor.block.heading',
  bulletList: 'editor.block.list',
  orderedList: 'editor.block.list',
  taskList: 'editor.block.list',
  blockquote: 'editor.block.quote',
  codeBlock: 'editor.block.code',
  table: 'editor.block.table',
  rawBlock: 'editor.block.html',
  horizontalRule: 'editor.block.rule',
};

export function useIssueText(): (issue: ProbeIssue) => string {
  const t = useT();
  return (issue) => {
    switch (issue.kind) {
      case 'unsupported':
        return t('editor.issue.unsupported', { tokens: issue.tokens.join(', ') });
      case 'invalid':
        return t('editor.issue.invalid');
      case 'frontMatter':
        return t('editor.issue.frontMatter');
      case 'changed':
        return t('editor.issue.changed', {
          block: t(BLOCK_KEYS[issue.type] ?? 'editor.block.other'),
          number: issue.block + 1,
        });
    }
  };
}

function ViewIssues({
  issues,
  opened,
  onClose,
}: {
  issues: ProbeIssue[];
  opened: boolean;
  onClose: () => void;
}) {
  const t = useT();
  const text = useIssueText();
  return (
    <div
      role="status"
      data-testid="view-issues"
      className="text-ui flex shrink-0 items-start gap-2 border-b border-muted/40 bg-sidebar px-4 py-2"
    >
      <p className="min-w-0 flex-1">
        {t(opened ? 'editor.openedInSource' : 'editor.richNotPossible')}{' '}
        {issues.map((issue) => text(issue)).join(' ')}
      </p>
      <button
        type="button"
        className="rounded-pw p-0.5 text-muted hover:text-fg"
        aria-label={t('editor.dismiss')}
        onClick={onClose}
      >
        <X aria-hidden="true" className="size-3.5" />
      </button>
    </div>
  );
}

function LossWarning({
  losses,
  onSaveAsPwrite,
  onSaveAnyway,
  onCancel,
}: {
  losses: Loss[];
  /** Library Markdown documents: keep everything as `.pwrite` (ADR 0015). */
  onSaveAsPwrite: (() => void) | null;
  onSaveAnyway: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  return (
    <div
      role="alert"
      data-testid="loss-warning"
      className="text-ui flex shrink-0 flex-col gap-2 border-b border-danger bg-sidebar px-4 py-2"
    >
      <p className="font-medium">{t('editor.losses.title')}</p>
      <ul className="list-disc pl-5">
        {losses.slice(0, 5).map((loss) => (
          <li key={`${loss.kind}-${String(loss.block)}`}>
            {t(LOSS_KEYS[loss.kind])}
            {loss.excerpt !== '' ? <span className="text-muted"> – „{loss.excerpt}“</span> : null}
          </li>
        ))}
        {losses.length > 5 ? <li>{t('editor.losses.more', { count: losses.length - 5 })}</li> : null}
      </ul>
      <div className="flex gap-2">
        {onSaveAsPwrite ? (
          <button
            type="button"
            data-testid="save-as-pwrite"
            className="inline-flex h-7 items-center rounded-pw bg-accent px-3 font-medium text-surface"
            onClick={onSaveAsPwrite}
          >
            {t('editor.losses.saveAsPwrite')}
          </button>
        ) : null}
        <button
          type="button"
          className={`inline-flex h-7 items-center rounded-pw px-3 ${
            onSaveAsPwrite ? 'border border-muted' : 'bg-accent font-medium text-surface'
          }`}
          onClick={onSaveAnyway}
        >
          {t('editor.losses.saveAnyway')}
        </button>
        <button
          type="button"
          className="inline-flex h-7 items-center rounded-pw border border-muted px-3"
          onClick={onCancel}
        >
          {t('editor.losses.cancel')}
        </button>
      </div>
    </div>
  );
}
