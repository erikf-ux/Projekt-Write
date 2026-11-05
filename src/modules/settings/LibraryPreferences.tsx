/**
 * Settings of the open library (ADR 0016): how long the trash keeps
 * documents and which versions the history keeps. Stored by Rust in
 * `application_settings`; each value is checked there again.
 */
import { useEffect, useState } from 'react';
import { errorText } from '@/components/errorText';
import { useT, type MessageKey } from '@/i18n';
import { useLibrary } from '@/modules/library/libraryStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { LibraryPreferences as Preferences } from '@/types/generated/LibraryPreferences';

const FIELDSET = 'mb-4 rounded-pw bg-sidebar p-4';
const INPUT = 'text-ui h-7 w-24 rounded-pw border border-muted bg-surface px-2 text-fg';

type Field = 'trashRetentionDays' | 'keepAllHours' | 'dailyDays' | 'weeklyDays';

const FIELDS: Record<Field, { labelKey: MessageKey; max: number; testId: string }> = {
  trashRetentionDays: { labelKey: 'settings.library.trashRetention', max: 3650, testId: 'trash-retention' },
  keepAllHours: { labelKey: 'settings.library.keepAllHours', max: 8760, testId: 'history-keep-all' },
  dailyDays: { labelKey: 'settings.library.dailyDays', max: 3650, testId: 'history-daily' },
  weeklyDays: { labelKey: 'settings.library.weeklyDays', max: 36500, testId: 'history-weekly' },
};

function valueOf(preferences: Preferences, field: Field): number {
  return field === 'trashRetentionDays' ? preferences.trashRetentionDays : preferences.history[field];
}

function withValue(preferences: Preferences, field: Field, value: number): Preferences {
  return field === 'trashRetentionDays'
    ? { ...preferences, trashRetentionDays: value }
    : { ...preferences, history: { ...preferences.history, [field]: value } };
}

export function LibraryPreferences() {
  const t = useT();
  const open = useLibrary((state) => state.status?.state === 'open');
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [drafts, setDrafts] = useState<Partial<Record<Field, string>>>({});
  const [error, setError] = useState<CommandError | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    let current = true;
    void call('library_preferences', {}).then((result) => {
      if (current && result.ok) {
        setPreferences(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, [open]);

  if (!open || !preferences) {
    return null;
  }

  const save = async (field: Field) => {
    const draft = drafts[field];
    setDrafts((previous) => ({ ...previous, [field]: undefined }));
    const value = Number(draft);
    if (draft === undefined || !Number.isInteger(value) || value === valueOf(preferences, field)) {
      return;
    }
    const result = await call('library_update_preferences', withValue(preferences, field, value));
    if (result.ok) {
      setPreferences(result.value);
      setError(null);
    } else {
      setError(result.error);
    }
  };

  const input = (field: Field) => {
    const { labelKey, max, testId } = FIELDS[field];
    return (
      <label className="text-ui mt-2 flex items-center gap-2" key={field}>
        <input
          type="number"
          min={1}
          max={max}
          className={INPUT}
          value={drafts[field] ?? String(valueOf(preferences, field))}
          data-testid={testId}
          onChange={(event) => {
            setDrafts((previous) => ({ ...previous, [field]: event.target.value }));
          }}
          onBlur={() => void save(field)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void save(field);
            }
          }}
        />
        {t(labelKey)}
      </label>
    );
  };

  return (
    <>
      <fieldset className={FIELDSET} data-testid="library-preferences">
        <legend className="text-ui font-semibold tracking-wide text-muted uppercase">
          {t('settings.library.preferences')}
        </legend>
        {input('trashRetentionDays')}
        <p className="text-ui mt-1 text-muted">{t('settings.library.trashRetentionHint')}</p>
      </fieldset>
      <fieldset className={FIELDSET} data-testid="history-preferences">
        <legend className="text-ui font-semibold tracking-wide text-muted uppercase">
          {t('settings.library.history')}
        </legend>
        {input('keepAllHours')}
        {input('dailyDays')}
        {input('weeklyDays')}
        <p className="text-ui mt-1 text-muted">{t('settings.library.historyHint')}</p>
      </fieldset>
      {error ? (
        <p role="alert" className="text-ui mb-4 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </>
  );
}
