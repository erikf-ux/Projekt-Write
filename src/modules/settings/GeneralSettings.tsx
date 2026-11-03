/** Language, design, line width and zoom (section 11). */
import { Trash2, Upload } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { errorText } from '@/components/errorText';
import { useT } from '@/i18n';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { LanguageSetting } from '@/types/generated/LanguageSetting';
import { nextZoom, useUi } from './uiStore';

const SECONDARY =
  'text-ui inline-flex h-7 items-center gap-2 rounded-pw border border-muted px-3 text-fg disabled:opacity-60';
const FIELDSET = 'mb-5 border-0 p-0';
const LEGEND = 'text-ui mb-2 font-semibold tracking-wide text-muted uppercase';

export function GeneralSettings() {
  const t = useT();
  const state = useUi((store) => store.state);
  const update = useUi((store) => store.update);
  const applyState = useUi((store) => store.applyState);
  const [themeError, setThemeError] = useState<CommandError | null>(null);
  const file = useRef<HTMLInputElement>(null);
  const name = useId();
  if (!state) {
    return null;
  }
  const { settings } = state;

  const languages: { value: LanguageSetting; label: string }[] = [
    { value: 'system', label: t('settings.language.system') },
    { value: 'de', label: 'Deutsch' },
    { value: 'en', label: 'English' },
  ];
  const themes = [
    { value: 'system', label: t('settings.theme.system') },
    { value: 'light', label: t('settings.theme.light') },
    { value: 'dark', label: t('settings.theme.dark') },
    ...state.customThemes.map((theme) => ({ value: `custom:${theme.id}`, label: theme.name, id: theme.id })),
  ];

  const importTheme = async (selected: File | undefined) => {
    setThemeError(null);
    if (!selected) {
      return;
    }
    const content = await selected.text();
    const result = await call('theme_import', { content });
    if (result.ok) {
      applyState(result.value);
    } else {
      setThemeError(result.error);
    }
  };

  const removeTheme = async (id: string) => {
    const result = await call('theme_remove', { id });
    if (result.ok) {
      applyState(result.value);
    } else {
      setThemeError(result.error);
    }
  };

  return (
    <div data-testid="settings-general">
      {state.readOnly ? (
        <p role="note" className="text-ui mb-4 text-danger">
          {t('error.settingsReadOnly')}
        </p>
      ) : null}
      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('settings.language')}</legend>
        <div className="flex flex-col gap-1">
          {languages.map((language) => (
            <label key={language.value} className="text-ui flex items-center gap-2">
              <input
                type="radio"
                name={`${name}-language`}
                value={language.value}
                checked={settings.language === language.value}
                data-testid={`language-${language.value}`}
                onChange={() => void update({ language: language.value })}
              />
              {language.label}
            </label>
          ))}
        </div>
      </fieldset>

      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('settings.theme')}</legend>
        <div className="flex flex-col gap-1">
          {themes.map((theme) => (
            <div key={theme.value} className="flex items-center gap-2">
              <label className="text-ui flex items-center gap-2">
                <input
                  type="radio"
                  name={`${name}-theme`}
                  value={theme.value}
                  checked={settings.theme === theme.value}
                  data-testid={`theme-${theme.value}`}
                  onChange={() => void update({ theme: theme.value })}
                />
                {theme.label}
              </label>
              {'id' in theme ? (
                <button
                  type="button"
                  className="inline-flex size-6 items-center justify-center rounded-pw text-muted hover:bg-surface hover:text-fg"
                  aria-label={t('settings.theme.remove', { name: theme.label })}
                  title={t('settings.theme.remove', { name: theme.label })}
                  onClick={() => void removeTheme(theme.id)}
                >
                  <Trash2 aria-hidden="true" className="size-3.5" />
                </button>
              ) : null}
            </div>
          ))}
        </div>
        <div className="mt-2">
          <button
            type="button"
            className={SECONDARY}
            data-testid="theme-import"
            onClick={() => {
              file.current?.click();
            }}
          >
            <Upload aria-hidden="true" className="size-3.5" />
            {t('settings.theme.import')}
          </button>
          <input
            ref={file}
            type="file"
            accept=".json,application/json"
            className="hidden"
            data-testid="theme-file"
            aria-hidden="true"
            tabIndex={-1}
            onChange={(event) => {
              void importTheme(event.target.files?.[0]);
              event.target.value = '';
            }}
          />
          <p className="text-ui mt-1 text-muted">{t('settings.theme.importHint')}</p>
          {themeError ? (
            <p role="alert" className="text-ui mt-1 text-danger" data-testid="theme-error">
              {errorText(t, themeError)}
            </p>
          ) : null}
        </div>
      </fieldset>

      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('settings.writing')}</legend>
        <label className="text-ui flex items-center gap-2">
          <input
            type="checkbox"
            checked={settings.limitLineWidth}
            data-testid="limit-line-width"
            onChange={(event) => void update({ limitLineWidth: event.target.checked })}
          />
          {t('settings.limitLineWidth')}
        </label>
      </fieldset>

      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('settings.zoom')}</legend>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className={SECONDARY}
            disabled={settings.zoom <= 0.5}
            onClick={() => void update({ zoom: nextZoom(settings.zoom, -1) })}
          >
            {t('command.zoom.out')}
          </button>
          <output className="text-ui min-w-14 text-center" data-testid="zoom-value">
            {Math.round(settings.zoom * 100)} %
          </output>
          <button
            type="button"
            className={SECONDARY}
            disabled={settings.zoom >= 2}
            onClick={() => void update({ zoom: nextZoom(settings.zoom, 1) })}
          >
            {t('command.zoom.in')}
          </button>
          <button
            type="button"
            className={SECONDARY}
            disabled={settings.zoom === 1}
            onClick={() => void update({ zoom: 1 })}
          >
            {t('command.zoom.reset')}
          </button>
        </div>
      </fieldset>
    </div>
  );
}
