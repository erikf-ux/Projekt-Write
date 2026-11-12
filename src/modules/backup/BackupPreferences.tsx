/**
 * Settings › Bibliothek › Backup (section 21, ADR 0021): target, the
 * warning for a target on the library's drive, the automatic backup, how
 * many are kept, the last backup, and the buttons for creating, checking
 * and restoring (stage 5).
 */
import { useEffect, useState } from 'react';
import { errorText } from '@/components/errorText';
import { useT } from '@/i18n';
import { useBackup } from './backupStore';

const FIELDSET = 'mb-4 rounded-pw bg-sidebar p-4';
const BUTTON =
  'text-ui inline-flex h-7 items-center rounded-pw border border-muted px-3 text-fg hover:bg-surface disabled:opacity-50';

export function BackupPreferences() {
  const t = useT();
  const overview = useBackup((state) => state.overview);
  const error = useBackup((state) => state.error);
  const busy = useBackup((state) => state.busy);
  const progress = useBackup((state) => state.progress);
  const [keep, setKeep] = useState<string | null>(null);

  useEffect(() => {
    void useBackup.getState().load();
  }, []);

  if (!overview) {
    return error ? (
      <p role="alert" className="text-ui mb-4 text-danger">
        {errorText(t, error)}
      </p>
    ) : null;
  }
  const store = useBackup.getState();
  const saveKeep = () => {
    if (keep === null) {
      return;
    }
    const value = Number.parseInt(keep, 10);
    setKeep(null);
    if (Number.isInteger(value) && value >= 1 && value <= 100 && value !== overview.keep) {
      void store.setOptions({ keep: value });
    }
  };

  return (
    <fieldset className={FIELDSET} data-testid="backup-preferences">
      <legend className="text-ui font-semibold tracking-wide text-muted uppercase">
        {t('settings.backup')}
      </legend>
      <p className="text-ui mt-2 break-all text-fg" data-testid="backup-target">
        {t('settings.backup.target', { target: overview.target })}{' '}
        {overview.defaultTarget ? (
          <span className="text-muted">{t('settings.backup.defaultTarget')}</span>
        ) : null}
      </p>
      {overview.sameDrive ? (
        <p role="alert" className="text-ui mt-1 text-danger" data-testid="backup-same-drive">
          {t('settings.backup.sameDrive')}
        </p>
      ) : null}
      <div className="mt-2 flex flex-wrap gap-2">
        <button
          type="button"
          className={BUTTON}
          data-testid="backup-choose-target"
          disabled={busy}
          onClick={() => void store.chooseTarget()}
        >
          {t('settings.backup.chooseTarget')}
        </button>
        {!overview.defaultTarget ? (
          <button
            type="button"
            className={BUTTON}
            data-testid="backup-reset-target"
            disabled={busy}
            onClick={() => void store.resetTarget()}
          >
            {t('settings.backup.resetTarget')}
          </button>
        ) : null}
      </div>
      <label className="text-ui mt-3 flex items-center gap-2 text-fg">
        <input
          type="checkbox"
          data-testid="backup-automatic"
          checked={overview.automatic}
          onChange={(event) => void store.setOptions({ automatic: event.target.checked })}
        />
        {t('settings.backup.automatic')}
      </label>
      <label className="text-ui mt-2 flex items-center gap-2 text-fg">
        <input
          type="number"
          min={1}
          max={100}
          className="text-ui h-7 w-24 rounded-pw border border-muted bg-surface px-2 text-fg"
          data-testid="backup-keep"
          value={keep ?? String(overview.keep)}
          onChange={(event) => {
            setKeep(event.target.value);
          }}
          onBlur={saveKeep}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              saveKeep();
            }
          }}
        />
        {t('settings.backup.keep')}
      </label>
      <p className="text-ui mt-2 text-muted" data-testid="backup-last">
        {overview.lastBackupAt
          ? t('settings.backup.last', { date: new Date(overview.lastBackupAt).toLocaleString() })
          : t('settings.backup.never')}
      </p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          type="button"
          className={BUTTON}
          data-testid="backup-create"
          disabled={busy || overview.running}
          onClick={() => void store.create()}
        >
          {t('command.backup.create')}
        </button>
        {busy && progress ? (
          <>
            <span className="text-ui text-muted" role="status" data-testid="backup-progress">
              {t('settings.backup.progress', { done: progress.done, total: progress.total })}
            </span>
            <button
              type="button"
              className={BUTTON}
              data-testid="backup-cancel"
              onClick={() => void store.cancel()}
            >
              {t('settings.backup.cancel')}
            </button>
          </>
        ) : null}
        <button
          type="button"
          className={BUTTON}
          data-testid="backup-check"
          disabled={busy}
          onClick={() => void store.check()}
        >
          {t('command.backup.check')}
        </button>
        <button
          type="button"
          className={BUTTON}
          data-testid="backup-restore"
          disabled={busy}
          onClick={() => void store.restore()}
        >
          {t('command.backup.restore')}
        </button>
      </div>
      <p className="text-ui mt-2 text-muted">{t('settings.backup.hint')}</p>
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </fieldset>
  );
}
