/**
 * Export, import and reset of the settings (ADR 0027, decision 4): the
 * texts of the import preview and what the running app does once Rust has
 * applied an import or a reset. Rust checks the file and keeps its values;
 * the interface only shows the changes and hands back the token.
 */
import type { Locale, MessageKey } from '@/i18n';
import { useBackup } from '@/modules/backup/backupStore';
import { useSuggestions } from '@/modules/mesh/suggestionStore';
import type { SettingsApplied } from '@/types/generated/SettingsApplied';
import type { SettingsArea } from '@/types/generated/SettingsArea';
import type { SettingsSkipReason } from '@/types/generated/SettingsSkipReason';
import { useUi } from './uiStore';

type Translate = (key: MessageKey, params?: Record<string, string | number>) => string;

/** The label of each key the settings file carries, by area. */
const LABELS: Record<SettingsArea, Record<string, MessageKey>> = {
  app: {
    language: 'settings.language',
    theme: 'settings.theme',
    zoom: 'settings.zoom',
    sidebar: 'settings.transfer.key.sidebar',
    limitLineWidth: 'settings.limitLineWidth',
    readingTime: 'settings.readingTime',
    autosaveExternal: 'settings.autosaveExternal',
    keybindings: 'settings.shortcuts',
    languageCheck: 'settings.language.check',
  },
  library: {
    'trash.retentionDays': 'settings.library.trashRetention',
    'backup.automatic': 'settings.backup.automatic',
    'backup.keep': 'settings.backup.keep',
    'history.keepAllHours': 'settings.library.keepAllHours',
    'history.dailyDays': 'settings.library.dailyDays',
    'history.weeklyDays': 'settings.library.weeklyDays',
    'mesh.suggestionThreshold': 'settings.transfer.key.meshThreshold',
    'media.maxMegapixels': 'settings.transfer.key.maxMegapixels',
  },
};

/** The readable name of a setting; the key itself for keys without a label. */
export function settingLabel(t: Translate, area: SettingsArea, key: string): string {
  const label = LABELS[area][key];
  return label ? t(label) : key;
}

export const SKIP_REASONS: Record<SettingsSkipReason, MessageKey> = {
  unknown: 'settings.transfer.skip.unknown',
  machine: 'settings.transfer.skip.machine',
  noLibrary: 'settings.transfer.skip.noLibrary',
};

/** A value of the preview as text: on/off, numbers in the interface language, objects as `name: value`. */
export function formatSettingValue(t: Translate, locale: Locale, key: string, value: unknown): string {
  if (key === 'zoom' && typeof value === 'number') {
    return `${String(Math.round(value * 100))} %`;
  }
  return formatValue(t, locale, value);
}

function formatValue(t: Translate, locale: Locale, value: unknown): string {
  if (value === null || value === undefined) {
    return '–';
  }
  if (typeof value === 'boolean') {
    return t(value ? 'settings.transfer.on' : 'settings.transfer.off');
  }
  if (typeof value === 'number') {
    return value.toLocaleString(locale);
  }
  if (typeof value === 'string') {
    return value;
  }
  if (Array.isArray(value)) {
    return value.length === 0 ? '–' : value.map((item) => formatValue(t, locale, item)).join(', ');
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value);
    return entries.length === 0
      ? '–'
      : entries.map(([name, item]) => `${name}: ${formatValue(t, locale, item)}`).join('; ');
  }
  return JSON.stringify(value);
}

/**
 * After an import or a reset: the interface takes the new settings at once
 * (theme, language, shortcuts, language check; Rust already set the zoom
 * and the menu), and views of library settings load them again.
 */
export function applySettingsResult(result: SettingsApplied): void {
  useUi.getState().applyState(result.ui);
  if (result.libraryChanged) {
    void useBackup.getState().load();
    if (useSuggestions.getState().open) {
      void useSuggestions.getState().load();
    }
  }
}
