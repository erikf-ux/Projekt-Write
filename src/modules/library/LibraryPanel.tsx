import { FolderOpen, FolderPlus, Info, MoveRight, RefreshCw, TriangleAlert } from 'lucide-react';
import { useEffect, useRef, type ReactNode } from 'react';
import { isMessageKey, useLocale, useT, type MessageKey } from '@/i18n';
import type { CommandError } from '@/types/generated/CommandError';
import type { LibraryInfo } from '@/types/generated/LibraryInfo';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import type { StorageLocation } from '@/types/generated/StorageLocation';
import { formatBytes } from './format';
import { useLibrary, type Prompt } from './libraryStore';

type Translate = ReturnType<typeof useT>;

const PRIMARY =
  'text-ui inline-flex h-7 items-center gap-2 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-60';
const SECONDARY =
  'text-ui inline-flex h-7 items-center gap-2 rounded-pw border border-muted px-3 text-fg disabled:opacity-60';

/** Translated message of a command error, with its details as parameters. */
function errorText(t: Translate, error: CommandError): string {
  const key = isMessageKey(error.messageKey) ? error.messageKey : 'error.internal';
  const params: Record<string, string> = {};
  for (const [name, value] of Object.entries(error.details ?? {})) {
    if (typeof value === 'string') {
      params[name] = value;
    }
  }
  return t(key, params);
}

function storageText(t: Translate, location: StorageLocation): string {
  switch (location.kind) {
    case 'local':
      return t('library.storage.local');
    case 'syncFolder':
      return t('library.storage.syncFolder', { provider: location.provider });
    case 'networkDrive':
      return t('library.storage.networkDrive');
  }
}

/** Title and explanation of the warning for a sync folder or network drive. */
function warningTexts(t: Translate, location: StorageLocation): { title: string; text: string } {
  if (location.kind === 'syncFolder') {
    return {
      title: t('library.warning.syncFolder.title'),
      text: t('library.warning.syncFolder', { provider: location.provider }),
    };
  }
  return { title: t('library.warning.networkDrive.title'), text: t('library.warning.networkDrive') };
}

export function LibraryPanel() {
  const t = useT();
  const status = useLibrary((state) => state.status);
  const defaultLocation = useLibrary((state) => state.defaultLocation);
  const busy = useLibrary((state) => state.busy);
  const error = useLibrary((state) => state.error);
  const prompt = useLibrary((state) => state.prompt);
  const progress = useLibrary((state) => state.progress);
  const moveNotice = useLibrary((state) => state.moveNotice);
  const load = useLibrary((state) => state.load);
  const createAtDefault = useLibrary((state) => state.createAtDefault);
  const chooseAndCreate = useLibrary((state) => state.chooseAndCreate);
  const chooseAndOpen = useLibrary((state) => state.chooseAndOpen);
  const chooseAndMove = useLibrary((state) => state.chooseAndMove);
  const retry = useLibrary((state) => state.retry);
  const acceptLocation = useLibrary((state) => state.acceptLocation);
  const cancelMove = useLibrary((state) => state.cancelMove);

  useEffect(() => {
    void load();
  }, [load]);

  const noticeKey = settingsNoticeKey(status);
  const openOther = () => void chooseAndOpen(t('library.dialog.open'));
  const createOther = () => void chooseAndCreate(t('library.dialog.create'));

  let content: ReactNode;
  if (status === null) {
    content = (
      <p role="status" className="text-ui text-muted">
        {t('library.loading')}
      </p>
    );
  } else if (status.state === 'moving') {
    content = <MovingView t={t} progress={progress} onCancel={() => void cancelMove()} />;
  } else if (status.state === 'open') {
    content = (
      <>
        <OpenView t={t} library={status.library} />
        {!status.library.locationAccepted && (
          <Notice icon="warning" data-testid="library-location-warning">
            <p className="font-semibold">{warningTexts(t, status.library.location).title}</p>
            <p>{warningTexts(t, status.library.location).text}</p>
            <p>{t('library.warning.inUse')}</p>
            <div className="mt-2 flex flex-wrap gap-2">
              <button
                type="button"
                className={SECONDARY}
                disabled={busy}
                onClick={() => void acceptLocation()}
              >
                {t('library.warning.dismiss')}
              </button>
            </div>
          </Notice>
        )}
        {moveNotice && (
          <Notice icon="info" data-testid="library-move-notice">
            <p>{t('library.moved.done', { path: moveNotice.oldPath })}</p>
          </Notice>
        )}
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            type="button"
            className={PRIMARY}
            disabled={busy}
            data-testid="library-move"
            onClick={() => void chooseAndMove(t('library.dialog.move'))}
          >
            <MoveRight aria-hidden="true" className="size-3.5" />
            {t('library.move')}
          </button>
          <button type="button" className={SECONDARY} disabled={busy} onClick={openOther}>
            <FolderOpen aria-hidden="true" className="size-3.5" />
            {t('library.openOther')}
          </button>
          <button type="button" className={SECONDARY} disabled={busy} onClick={createOther}>
            <FolderPlus aria-hidden="true" className="size-3.5" />
            {t('library.createNew')}
          </button>
        </div>
      </>
    );
  } else {
    content = (
      <>
        {status.error ? (
          <>
            <p className="text-ui text-danger" data-testid="library-open-error">
              {errorText(t, status.error)}
            </p>
            {status.configuredPath !== null && (
              <p className="text-ui mt-1 break-all text-muted">
                {t('library.configured', { path: status.configuredPath })}
              </p>
            )}
          </>
        ) : (
          <p className="text-ui text-fg">{t('library.intro')}</p>
        )}
        <div className="mt-4 flex flex-wrap gap-2">
          {status.error && status.configuredPath !== null && (
            <button type="button" className={PRIMARY} disabled={busy} onClick={() => void retry()}>
              <RefreshCw aria-hidden="true" className="size-3.5" />
              {t('library.retry')}
            </button>
          )}
          {defaultLocation && (
            <button
              type="button"
              className={status.error ? SECONDARY : PRIMARY}
              disabled={busy}
              data-testid="library-create-default"
              onClick={() => void createAtDefault()}
            >
              <FolderPlus aria-hidden="true" className="size-3.5" />
              {t('library.create')}
            </button>
          )}
          <button type="button" className={SECONDARY} disabled={busy} onClick={createOther}>
            {t('library.chooseOther')}
          </button>
          <button type="button" className={SECONDARY} disabled={busy} onClick={openOther}>
            <FolderOpen aria-hidden="true" className="size-3.5" />
            {t('library.openExisting')}
          </button>
        </div>
        {defaultLocation && (
          <p className="text-ui mt-2 break-all text-muted" data-testid="library-default-location">
            {t('library.createAt', { path: defaultLocation.displayPath })}
          </p>
        )}
      </>
    );
  }

  return (
    <section
      aria-labelledby="library-title"
      className="mb-4 rounded-pw bg-sidebar p-4"
      data-testid="library"
      data-state={status?.state ?? 'loading'}
      aria-busy={busy}
    >
      <h2 id="library-title" className="text-ui mb-2 font-semibold tracking-wide text-muted uppercase">
        {t('library.title')}
      </h2>
      {content}
      {error && (
        <p role="alert" className="text-ui mt-3 text-danger" data-testid="library-error">
          {errorText(t, error)}
        </p>
      )}
      {noticeKey && (
        <p className="text-ui mt-3 text-muted" data-testid="library-settings-notice">
          {t(noticeKey)}
        </p>
      )}
      {prompt && <PromptDialog t={t} prompt={prompt} />}
    </section>
  );
}

function settingsNoticeKey(status: ReturnType<typeof useLibrary.getState>['status']): MessageKey | null {
  if (status === null || status.state === 'moving' || status.settingsNotice === null) {
    return null;
  }
  const keys: Record<NonNullable<typeof status.settingsNotice>, MessageKey> = {
    damaged: 'library.settings.damaged',
    newerVersion: 'library.settings.newerVersion',
    notSaved: 'library.settings.notSaved',
  };
  return keys[status.settingsNotice];
}

function OpenView({ t, library }: { t: Translate; library: LibraryInfo }) {
  const notes = library.notes;
  return (
    <>
      <p className="text-[16px] leading-tight font-semibold text-fg" data-testid="library-name">
        {library.name}
      </p>
      <dl className="text-ui mt-2 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1">
        <dt className="text-muted">{t('library.folder')}</dt>
        <dd className="break-all text-fg" data-testid="library-path">
          {library.displayPath}
        </dd>
        <dt className="text-muted">{t('library.storage')}</dt>
        <dd className="text-fg" data-testid="library-storage">
          {storageText(t, library.location)}
        </dd>
        <dt className="text-muted">{t('library.journal')}</dt>
        <dd className="text-fg" data-testid="library-journal">
          {t(library.journalMode === 'wal' ? 'library.journal.wal' : 'library.journal.rollback')}
        </dd>
        <dt className="text-muted">{t('library.documents')}</dt>
        <dd className="text-fg" data-testid="library-documents">
          {library.documentCount}
        </dd>
      </dl>
      {notes.migratedFrom !== null && notes.migrationBackup !== null && (
        <Notice icon="info">
          <p>{t('library.notes.migrated', { from: notes.migratedFrom, backup: notes.migrationBackup })}</p>
        </Notice>
      )}
      {notes.orphanedFiles > 0 && (
        <Notice icon="warning">
          <p>{t('library.notes.orphaned', { count: notes.orphanedFiles })}</p>
        </Notice>
      )}
      {notes.missingFiles > 0 && (
        <Notice icon="warning">
          <p>{t('library.notes.missing', { count: notes.missingFiles })}</p>
        </Notice>
      )}
    </>
  );
}

function MovingView({
  t,
  progress,
  onCancel,
}: {
  t: Translate;
  progress: MoveProgress | null;
  onCancel: () => void;
}) {
  const locale = useLocale((state) => state.locale);
  let text = t('library.moving.preparing');
  if (progress) {
    const bytes = formatBytes(progress.doneBytes, locale);
    const totalBytes = formatBytes(progress.totalBytes, locale);
    const params = {
      files: progress.doneFiles,
      totalFiles: progress.totalFiles,
      bytes: t(bytes.key, { value: bytes.value }),
      totalBytes: t(totalBytes.key, { value: totalBytes.value }),
    };
    text =
      progress.phase === 'copying'
        ? t('library.moving.copying', params)
        : progress.phase === 'verifying'
          ? t('library.moving.verifying', params)
          : t('library.moving.switching');
  }
  const max = progress ? Math.max(progress.totalBytes, 1) : undefined;
  const value = progress ? Math.min(progress.doneBytes, max ?? 0) : undefined;
  return (
    <div data-testid="library-moving">
      <p className="text-ui font-semibold text-fg">{t('library.moving.title')}</p>
      <p role="status" aria-live="polite" className="text-ui mt-1 text-fg" data-testid="library-progress">
        {text}
      </p>
      <progress className="mt-2 h-2 w-full accent-accent" max={max} value={value}>
        {text}
      </progress>
      <div className="mt-3">
        <button type="button" className={SECONDARY} onClick={onCancel}>
          {t('library.cancel')}
        </button>
      </div>
    </div>
  );
}

function Notice({
  icon,
  children,
  ...rest
}: {
  icon: 'warning' | 'info';
  children: ReactNode;
  'data-testid'?: string;
}) {
  const Icon = icon === 'warning' ? TriangleAlert : Info;
  return (
    <div className="text-ui mt-3 flex gap-2 rounded-pw border border-muted p-3 text-fg" {...rest}>
      <Icon
        aria-hidden="true"
        className={`mt-0.5 size-4 shrink-0 ${icon === 'warning' ? 'text-danger' : 'text-accent'}`}
      />
      <div className="flex flex-col gap-1">{children}</div>
    </div>
  );
}

/** Modal confirmation. Escape cancels; focus starts on the main choice. */
function PromptDialog({ t, prompt }: { t: Translate; prompt: Prompt }) {
  const confirmPrompt = useLibrary((state) => state.confirmPrompt);
  const dismissPrompt = useLibrary((state) => state.dismissPrompt);
  const chooseAndCreate = useLibrary((state) => state.chooseAndCreate);
  const chooseAndMove = useLibrary((state) => state.chooseAndMove);
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    confirmRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        dismissPrompt();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [dismissPrompt]);

  let title: string;
  let body: ReactNode;
  let confirmLabel: string;
  let chooseOther: (() => void) | null = null;
  switch (prompt.kind) {
    case 'confirmLocation': {
      const texts = warningTexts(t, prompt.choice.location);
      title = texts.title;
      body = (
        <>
          <p>{texts.text}</p>
          <p>{t('library.warning.rollback')}</p>
          <p className="break-all text-muted">{prompt.choice.displayPath}</p>
        </>
      );
      confirmLabel = t(
        prompt.action === 'create' ? 'library.warning.createAnyway' : 'library.warning.moveAnyway',
      );
      const action = prompt.action;
      chooseOther = () => {
        dismissPrompt();
        void (action === 'create'
          ? chooseAndCreate(t('library.dialog.create'))
          : chooseAndMove(t('library.dialog.move')));
      };
      break;
    }
    case 'existingLibrary':
      title = t('library.choice.isLibrary');
      body = <p className="break-all text-muted">{prompt.choice.displayPath}</p>;
      confirmLabel = t('library.choice.openInstead');
      break;
    case 'movedLibrary':
      title = t('library.moved.title');
      body = <p className="break-all">{t('library.moved.text', { path: prompt.movedTo })}</p>;
      confirmLabel = t('library.moved.openAnyway');
      break;
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-surface/80 p-6">
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="library-prompt-title"
        aria-describedby="library-prompt-body"
        className="w-full max-w-[480px] rounded-pw border border-muted bg-sidebar p-4 text-fg"
        data-testid="library-prompt"
        data-kind={prompt.kind}
      >
        <h3 id="library-prompt-title" className="text-ui mb-2 flex items-center gap-2 font-semibold">
          <TriangleAlert aria-hidden="true" className="size-4 shrink-0 text-danger" />
          {title}
        </h3>
        <div id="library-prompt-body" className="text-ui flex flex-col gap-2">
          {body}
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            ref={confirmRef}
            type="button"
            className={PRIMARY}
            data-testid="library-prompt-confirm"
            onClick={() => void confirmPrompt()}
          >
            {confirmLabel}
          </button>
          {chooseOther && (
            <button
              type="button"
              className={SECONDARY}
              data-testid="library-prompt-other"
              onClick={chooseOther}
            >
              {t('library.warning.chooseOther')}
            </button>
          )}
          <button
            type="button"
            className={SECONDARY}
            data-testid="library-prompt-cancel"
            onClick={dismissPrompt}
          >
            {t('library.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}
