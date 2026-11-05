/**
 * Stage 2, "Absturz" (ADR 0016, decision 9): after a start the drafts of
 * changes that were never saved are offered. Restoring a library document
 * keeps its current state as a version first (Rust); an external file opens
 * with the draft as an unsaved change; a draft of a vanished file becomes a
 * library document. "Später" keeps the drafts for the next start.
 */
import { useState } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useLocale, useT } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DraftEntry } from '@/types/generated/DraftEntry';

interface RecoveryStore {
  entries: DraftEntry[];
  error: CommandError | null;
}

export const useRecovery = create<RecoveryStore>(() => ({ entries: [], error: null }));

/** Asks Rust for drafts to offer (after the library has opened). */
export async function offerRecovery(): Promise<void> {
  const result = await call('recovery_list', {});
  useRecovery.setState({ entries: result.ok ? result.value : [], error: null });
}

function done(key: string): void {
  useRecovery.setState((state) => ({ entries: state.entries.filter((entry) => entry.key !== key) }));
}

/** Restores one draft and shows it in a tab. */
export async function restoreDraft(entry: DraftEntry): Promise<boolean> {
  const result = await call('recovery_restore', { key: entry.key });
  if (!result.ok) {
    useRecovery.setState({ error: result.error });
    return false;
  }
  const workspace = useWorkspace.getState();
  const restored = result.value;
  if (restored.kind === 'external') {
    await workspace.openExternal(restored.file, restored.text);
  } else {
    workspace.addDocument(restored.document);
    if (workspace.open[restored.document.id]) {
      await workspace.reloadLibraryDocument(restored.document.id);
      workspace.activate(restored.document.id);
    } else {
      await workspace.openDocument(restored.document.id);
    }
  }
  done(entry.key);
  return true;
}

export async function dropDraft(entry: DraftEntry): Promise<void> {
  const result = await call('recovery_drop', { key: entry.key });
  if (result.ok) {
    done(entry.key);
  } else {
    useRecovery.setState({ error: result.error });
  }
}

const BUTTON = 'text-ui inline-flex h-7 items-center rounded-pw px-3';

export function RecoveryDialog() {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const entries = useRecovery((state) => state.entries);
  const error = useRecovery((state) => state.error);
  const [busy, setBusy] = useState(false);
  if (entries.length === 0) {
    return null;
  }
  const dateTime = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'medium' });
  const later = () => {
    useRecovery.setState({ entries: [], error: null });
  };
  const all = async (action: (entry: DraftEntry) => Promise<unknown>) => {
    setBusy(true);
    for (const entry of [...useRecovery.getState().entries]) {
      await action(entry);
    }
    setBusy(false);
  };
  return (
    <Dialog title={t('recovery.title')} onClose={later} testId="recovery-dialog" width="w-[min(620px,94vw)]">
      <p className="text-ui">{t('recovery.text')}</p>
      <ul className="mt-3 flex flex-col gap-2">
        {entries.map((entry) => (
          <li
            key={entry.key}
            className="text-ui flex items-center gap-3 rounded-pw border border-muted/40 px-3 py-2"
            data-draft={entry.key}
          >
            <div className="min-w-0 flex-1">
              <p className="truncate font-medium">{entry.title}</p>
              <p className="text-muted">
                {t('recovery.changedAt', { date: dateTime.format(new Date(entry.updatedAt)) })}
                {entry.external
                  ? ` · ${t(entry.missing ? 'recovery.fileMissing' : 'recovery.externalFile')}`
                  : ''}
              </p>
            </div>
            <button
              type="button"
              className={`${BUTTON} bg-accent font-medium text-surface`}
              data-choice="restore"
              disabled={busy}
              onClick={() => void restoreDraft(entry)}
            >
              {t('recovery.restore')}
            </button>
            <button
              type="button"
              className={`${BUTTON} border border-muted`}
              data-choice="drop"
              disabled={busy}
              onClick={() => void dropDraft(entry)}
            >
              {t('recovery.drop')}
            </button>
          </li>
        ))}
      </ul>
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        <button type="button" className={`${BUTTON} border border-muted`} disabled={busy} onClick={later}>
          {t('recovery.later')}
        </button>
        {entries.length > 1 ? (
          <>
            <button
              type="button"
              className={`${BUTTON} border border-danger text-danger`}
              disabled={busy}
              onClick={() => void all(dropDraft)}
            >
              {t('recovery.dropAll')}
            </button>
            <button
              type="button"
              className={`${BUTTON} bg-accent font-medium text-surface`}
              disabled={busy}
              data-choice="restoreAll"
              onClick={() => void all(restoreDraft)}
            >
              {t('recovery.restoreAll')}
            </button>
          </>
        ) : null}
      </div>
    </Dialog>
  );
}
