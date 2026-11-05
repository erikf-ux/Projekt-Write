/**
 * Settings of the open library (ADR 0016): how long the trash keeps
 * documents. Stored by Rust in `application_settings`; each value is
 * checked there again.
 */
import { useEffect, useState } from 'react';
import { errorText } from '@/components/errorText';
import { useT } from '@/i18n';
import { useLibrary } from '@/modules/library/libraryStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { LibraryPreferences as Preferences } from '@/types/generated/LibraryPreferences';

const FIELDSET = 'mb-4 rounded-pw bg-sidebar p-4';
const INPUT = 'text-ui h-7 w-24 rounded-pw border border-muted bg-surface px-2 text-fg';

export function LibraryPreferences() {
  const t = useT();
  const open = useLibrary((state) => state.status?.state === 'open');
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<CommandError | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    let current = true;
    void call('library_preferences', {}).then((result) => {
      if (current && result.ok) {
        setPreferences(result.value);
        setDraft(String(result.value.trashRetentionDays));
      }
    });
    return () => {
      current = false;
    };
  }, [open]);

  if (!open || !preferences) {
    return null;
  }

  const save = async () => {
    const days = Number(draft);
    if (!Number.isInteger(days) || days === preferences.trashRetentionDays) {
      setDraft(String(preferences.trashRetentionDays));
      return;
    }
    const result = await call('library_update_preferences', { ...preferences, trashRetentionDays: days });
    if (result.ok) {
      setPreferences(result.value);
      setDraft(String(result.value.trashRetentionDays));
      setError(null);
    } else {
      setError(result.error);
    }
  };

  return (
    <fieldset className={FIELDSET} data-testid="library-preferences">
      <legend className="text-ui font-semibold tracking-wide text-muted uppercase">
        {t('settings.library.preferences')}
      </legend>
      <label className="text-ui mt-2 flex items-center gap-2">
        <input
          type="number"
          min={1}
          max={3650}
          className={INPUT}
          value={draft}
          data-testid="trash-retention"
          aria-describedby="pw-trash-retention-hint"
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={() => void save()}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void save();
            }
          }}
        />
        {t('settings.library.trashRetention')}
      </label>
      <p id="pw-trash-retention-hint" className="text-ui mt-1 text-muted">
        {t('settings.library.trashRetentionHint')}
      </p>
      {error ? (
        <p role="alert" className="text-ui mt-1 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </fieldset>
  );
}
