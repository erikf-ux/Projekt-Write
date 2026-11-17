/**
 * Settings › Erweitert (ADR 0027, decision 5): export, import and reset of
 * the settings, the built-in extensions and the storage locations.
 */
import { useEffect, useState } from 'react';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { ask } from '@/components/prompt/promptStore';
import { useLocale, useT, type MessageKey } from '@/i18n';
import { useLibrary } from '@/modules/library/libraryStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { CustomTheme } from '@/types/generated/CustomTheme';
import type { SettingsArea } from '@/types/generated/SettingsArea';
import type { SettingsImportPreview } from '@/types/generated/SettingsImportPreview';
import type { StorageFolder } from '@/types/generated/StorageFolder';
import type { StorageLocations } from '@/types/generated/StorageLocations';
import { applySettingsResult, formatSettingValue, settingLabel, SKIP_REASONS } from './settingsTransfer';
import { useUi } from './uiStore';

const FIELDSET = 'mb-4 rounded-pw bg-sidebar p-4';
const LEGEND = 'text-ui font-semibold tracking-wide text-muted uppercase';
const BUTTON =
  'text-ui inline-flex h-7 items-center rounded-pw border border-muted px-3 text-fg hover:bg-surface disabled:opacity-50';
const PRIMARY =
  'text-ui inline-flex h-7 items-center rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-50';

const NO_THEMES: readonly CustomTheme[] = [];

const LOCATIONS: { id: StorageFolder; labelKey: MessageKey }[] = [
  { id: 'config', labelKey: 'settings.locations.config' },
  { id: 'logs', labelKey: 'settings.locations.logs' },
  { id: 'library', labelKey: 'settings.locations.library' },
];

const RESETS: Record<SettingsArea, { titleKey: MessageKey; textKey: MessageKey; doneKey: MessageKey }> = {
  app: {
    titleKey: 'settings.reset.appTitle',
    textKey: 'settings.reset.appText',
    doneKey: 'settings.reset.appDone',
  },
  library: {
    titleKey: 'settings.reset.libraryTitle',
    textKey: 'settings.reset.libraryText',
    doneKey: 'settings.reset.libraryDone',
  },
};

export function AdvancedSettings() {
  const t = useT();
  const libraryOpen = useLibrary((state) => state.status?.state === 'open');
  const [locations, setLocations] = useState<StorageLocations | null>(null);
  const [preview, setPreview] = useState<SettingsImportPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<MessageKey | null>(null);
  const [error, setError] = useState<CommandError | null>(null);

  useEffect(() => {
    let current = true;
    void call('system_locations', {}).then((result) => {
      if (current && result.ok) {
        setLocations(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, [libraryOpen]);

  /** Runs one action at a time and shows its outcome: a message or an error. */
  const run = async (action: () => Promise<MessageKey | null | CommandError>) => {
    setBusy(true);
    setMessage(null);
    setError(null);
    const outcome = await action();
    setBusy(false);
    if (typeof outcome === 'string') {
      setMessage(outcome);
    } else if (outcome) {
      setError(outcome);
    }
  };

  const exportSettings = () =>
    run(async () => {
      const result = await call('settings_export', { title: t('settings.transfer.exportTitle') });
      if (!result.ok) {
        return result.error;
      }
      return result.value ? 'settings.transfer.exported' : null;
    });

  const importSettings = () =>
    run(async () => {
      const result = await call('settings_import_preview', { title: t('settings.transfer.importTitle') });
      if (!result.ok) {
        return result.error;
      }
      setPreview(result.value);
      return null;
    });

  const applyImport = (token: string) =>
    run(async () => {
      setPreview(null);
      const result = await call('settings_import_apply', { token });
      if (!result.ok) {
        return result.error;
      }
      applySettingsResult(result.value);
      return 'settings.transfer.imported';
    });

  const reset = async (area: SettingsArea) => {
    const texts = RESETS[area];
    const answer = await ask({
      titleKey: texts.titleKey,
      textKey: texts.textKey,
      choices: [
        { id: 'reset', labelKey: 'settings.reset.confirm', tone: 'danger' },
        { id: 'cancel', labelKey: 'settings.transfer.cancel' },
      ],
      cancel: 'cancel',
      testId: 'settings-reset-confirm',
    });
    if (answer !== 'reset') {
      return;
    }
    await run(async () => {
      const result = await call('settings_reset', { area });
      if (!result.ok) {
        return result.error;
      }
      applySettingsResult(result.value);
      return texts.doneKey;
    });
  };

  const openLocation = async (location: StorageFolder) => {
    setError(null);
    const result = await call('system_open_location', { location });
    if (!result.ok) {
      setError(result.error);
    }
  };

  return (
    <div data-testid="settings-advanced">
      <fieldset className={FIELDSET} data-testid="settings-transfer">
        <legend className={LEGEND}>{t('settings.transfer')}</legend>
        <div className="mt-2 flex flex-wrap gap-2">
          <button
            type="button"
            className={BUTTON}
            data-testid="settings-export"
            disabled={busy}
            onClick={() => void exportSettings()}
          >
            {t('settings.transfer.export')}
          </button>
          <button
            type="button"
            className={BUTTON}
            data-testid="settings-import"
            disabled={busy}
            onClick={() => void importSettings()}
          >
            {t('settings.transfer.import')}
          </button>
        </div>
        <p className="text-ui mt-2 text-muted">{t('settings.transfer.hint')}</p>
      </fieldset>

      <fieldset className={FIELDSET} data-testid="settings-reset">
        <legend className={LEGEND}>{t('settings.reset')}</legend>
        <div className="mt-2 flex flex-wrap gap-2">
          <button
            type="button"
            className={BUTTON}
            data-testid="settings-reset-app"
            disabled={busy}
            onClick={() => void reset('app')}
          >
            {t('settings.reset.app')}
          </button>
          <button
            type="button"
            className={BUTTON}
            data-testid="settings-reset-library"
            disabled={busy || !libraryOpen}
            onClick={() => void reset('library')}
          >
            {t('settings.reset.library')}
          </button>
        </div>
        <p className="text-ui mt-2 text-muted">{t('settings.reset.hint')}</p>
      </fieldset>

      {message ? (
        <p role="status" className="text-ui mb-4 text-fg" data-testid="settings-advanced-message">
          {t(message)}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-ui mb-4 text-danger" data-testid="settings-advanced-error">
          {errorText(t, error)}
        </p>
      ) : null}

      {/*
        Built-in extensions per registry (ADR 0027, decision 5): read-only,
        origin "eingebaut", from `extensions_list`. Filled in by the
        registries' part of Phase 13.
      */}
      <div data-testid="advanced-extensions" />

      <fieldset className={FIELDSET} data-testid="settings-locations">
        <legend className={LEGEND}>{t('settings.locations')}</legend>
        <dl className="mt-2 flex flex-col gap-2">
          {LOCATIONS.map(({ id, labelKey }) => {
            const path = locations?.[id] ?? null;
            return (
              <div key={id} className="flex flex-wrap items-center gap-2" data-testid={`location-${id}`}>
                <dt className="text-ui w-28 shrink-0 text-muted">{t(labelKey)}</dt>
                <dd className="text-ui min-w-0 flex-1 break-all text-fg" data-testid={`location-${id}-path`}>
                  {path ?? (id === 'library' ? t('settings.locations.noLibrary') : '…')}
                </dd>
                <dd>
                  <button
                    type="button"
                    className={BUTTON}
                    data-testid={`location-${id}-open`}
                    aria-label={t('settings.locations.openLabel', { name: t(labelKey) })}
                    disabled={path === null}
                    onClick={() => void openLocation(id)}
                  >
                    {t('settings.locations.open')}
                  </button>
                </dd>
              </div>
            );
          })}
        </dl>
      </fieldset>

      {preview ? (
        <ImportPreviewDialog
          preview={preview}
          busy={busy}
          onApply={() => void applyImport(preview.token)}
          onCancel={() => {
            setPreview(null);
          }}
        />
      ) : null}
    </div>
  );
}

interface ImportPreviewDialogProps {
  preview: SettingsImportPreview;
  busy: boolean;
  onApply: () => void;
  onCancel: () => void;
}

/** What the import changes and what it skips; "Übernehmen" or "Abbrechen". */
export function ImportPreviewDialog({ preview, busy, onApply, onCancel }: ImportPreviewDialogProps) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const customThemes = useUi((state) => state.state?.customThemes ?? NO_THEMES);
  const areas: SettingsArea[] = ['app', 'library'];
  return (
    <Dialog title={t('settings.transfer.preview.title')} onClose={onCancel} testId="settings-import-preview">
      {preview.exportedAt ? (
        <p className="text-ui mb-2 text-muted">
          {t('settings.transfer.preview.exportedAt', {
            date: new Date(preview.exportedAt).toLocaleString(locale),
          })}
        </p>
      ) : null}
      {preview.changes.length === 0 ? (
        <p className="text-ui mb-2" data-testid="settings-import-none">
          {t('settings.transfer.preview.none')}
        </p>
      ) : (
        areas.map((area) => {
          const changes = preview.changes.filter((change) => change.area === area);
          if (changes.length === 0) {
            return null;
          }
          return (
            <table
              key={area}
              className="text-ui mb-3 w-full border-collapse"
              data-testid={`settings-changes-${area}`}
            >
              <caption className="mb-1 text-left font-semibold">
                {t(`settings.transfer.area.${area}`)}
              </caption>
              <thead>
                <tr className="text-left text-muted">
                  <th scope="col" className="pr-2 font-normal">
                    {t('settings.transfer.preview.setting')}
                  </th>
                  <th scope="col" className="pr-2 font-normal">
                    {t('settings.transfer.preview.before')}
                  </th>
                  <th scope="col" className="font-normal">
                    {t('settings.transfer.preview.after')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {changes.map((change) => (
                  <tr key={change.key} data-key={change.key} className="border-t border-muted/30 align-top">
                    <th scope="row" className="py-1 pr-2 text-left font-normal">
                      {settingLabel(t, area, change.key)}
                    </th>
                    <td className="py-1 pr-2 break-all text-muted">
                      {formatSettingValue(t, locale, change.key, change.before, customThemes)}
                    </td>
                    <td className="py-1 break-all">
                      {formatSettingValue(t, locale, change.key, change.after, customThemes)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          );
        })
      )}
      {preview.skipped.length > 0 ? (
        <div className="mb-3" data-testid="settings-import-skipped">
          <p className="text-ui font-semibold">{t('settings.transfer.preview.skipped')}</p>
          <ul className="text-ui list-disc pl-5 text-muted">
            {preview.skipped.map((entry) => (
              <li key={entry.key}>
                <code>{entry.key}</code> – {t(SKIP_REASONS[entry.reason])}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <p className="text-ui text-muted">{t('settings.transfer.preview.backup')}</p>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        <button
          type="button"
          className={PRIMARY}
          data-autofocus
          data-testid="settings-import-apply"
          disabled={busy}
          onClick={onApply}
        >
          {t('settings.transfer.apply')}
        </button>
        <button type="button" className={BUTTON} data-testid="settings-import-cancel" onClick={onCancel}>
          {t('settings.transfer.cancel')}
        </button>
      </div>
    </Dialog>
  );
}
