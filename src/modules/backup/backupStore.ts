/**
 * Backups in the interface (Phase 10, section 21, ADR 0021): the overview
 * of the settings, "Backup erstellen" with progress, "Backup prüfen" and
 * the restore of stage 5 "Backup".
 */
import { create } from 'zustand';
import { translate, useLocale } from '@/i18n';
import { useLibrary } from '@/modules/library/libraryStore';
import { useUi } from '@/modules/settings/uiStore';
import { call, callWithProgress } from '@/services/ipc';
import type { BackupOverview } from '@/types/generated/BackupOverview';
import type { BackupProgress } from '@/types/generated/BackupProgress';
import type { CommandError } from '@/types/generated/CommandError';

const t = (key: Parameters<typeof translate>[1]) => translate(useLocale.getState().locale, key);

interface BackupStore {
  overview: BackupOverview | null;
  error: CommandError | null;
  /** A backup, check or restore in progress. */
  progress: BackupProgress | null;
  busy: boolean;
  load: () => Promise<void>;
  chooseTarget: () => Promise<void>;
  resetTarget: () => Promise<void>;
  setOptions: (options: { automatic?: boolean; keep?: number }) => Promise<void>;
  create: () => Promise<boolean>;
  cancel: () => Promise<void>;
  check: () => Promise<boolean>;
  restore: () => Promise<boolean>;
}

export const useBackup = create<BackupStore>((set, get) => {
  const apply = (result: Awaited<ReturnType<typeof call<'backup_overview'>>>) => {
    if (result.ok) {
      set({ overview: result.value, error: null });
    } else {
      set({ error: result.error });
    }
  };
  const onProgress = (progress: BackupProgress) => {
    set({ progress });
  };
  return {
    overview: null,
    error: null,
    progress: null,
    busy: false,

    load: async () => {
      apply(await call('backup_overview', {}));
    },

    chooseTarget: async () => {
      const result = await call('backup_choose_target', { title: t('backup.chooseTarget') });
      if (result.ok) {
        if (result.value) {
          set({ overview: result.value, error: null });
        }
      } else {
        set({ error: result.error });
      }
    },

    resetTarget: async () => {
      apply(await call('backup_reset_target', {}));
    },

    setOptions: async (options) => {
      apply(await call('backup_set_options', options));
    },

    create: async () => {
      if (get().busy) {
        return false;
      }
      set({ busy: true, progress: null, error: null });
      useUi.getState().showNotice({ tone: 'info', key: 'backup.running' });
      const result = await callWithProgress('backup_create', {}, onProgress);
      set({ busy: false, progress: null });
      await get().load();
      if (!result.ok) {
        useUi
          .getState()
          .showNotice(
            result.error.code === 'BACKUP_CANCELLED'
              ? { tone: 'info', key: 'backup.cancelled' }
              : { tone: 'error', key: 'backup.failed', error: result.error },
          );
        return false;
      }
      useUi.getState().showNotice({
        tone: 'info',
        key: 'backup.done',
        params: { documents: result.value.documents, media: result.value.media },
      });
      return true;
    },

    cancel: async () => {
      await call('backup_cancel', {});
    },

    check: async () => {
      if (get().busy) {
        return false;
      }
      set({ busy: true, progress: null });
      const result = await callWithProgress('backup_check', { title: t('command.backup.check') }, onProgress);
      set({ busy: false, progress: null });
      if (!result.ok) {
        useUi.getState().showNotice({ tone: 'error', key: 'backup.checkFailed', error: result.error });
        return false;
      }
      if (!result.value) {
        return false;
      }
      useUi.getState().showNotice({
        tone: 'info',
        key: 'backup.checkPassed',
        params: {
          documents: result.value.documents,
          media: result.value.media,
          date: result.value.createdAt.slice(0, 10),
        },
      });
      return true;
    },

    restore: async () => {
      if (get().busy) {
        return false;
      }
      set({ busy: true, progress: null });
      const result = await callWithProgress(
        'backup_restore',
        { fileTitle: t('backup.restoreFile'), folderTitle: t('backup.restoreFolder') },
        onProgress,
      );
      set({ busy: false, progress: null });
      if (!result.ok) {
        useUi.getState().showNotice({ tone: 'error', key: 'backup.restoreFailed', error: result.error });
        return false;
      }
      if (!result.value) {
        return false;
      }
      // The restored library is the open one now; the old one stays.
      useLibrary.setState({ status: result.value });
      useUi.getState().showNotice({ tone: 'info', key: 'backup.restored' });
      await get().load();
      return true;
    },
  };
});
