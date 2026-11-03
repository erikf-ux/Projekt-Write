/** Settings: general, shortcuts, library (section 11; library from Phase 2). */
import { useCallback, useMemo } from 'react';
import { Dialog } from '@/components/Dialog';
import { useT, type MessageKey } from '@/i18n';
import { effectiveKeybindings } from '@/modules/commands/keymap';
import { allCommands } from '@/modules/commands/registry';
import { LibraryPanel } from '@/modules/library/LibraryPanel';
import { GeneralSettings } from './GeneralSettings';
import { ShortcutSettings } from './ShortcutSettings';
import { useUi, type SettingsSection } from './uiStore';

const sections: { id: SettingsSection; key: MessageKey }[] = [
  { id: 'general', key: 'settings.general' },
  { id: 'shortcuts', key: 'settings.shortcuts' },
  { id: 'library', key: 'settings.library' },
];

export function SettingsDialog() {
  const t = useT();
  const section = useUi((store) => store.settingsSection);
  const state = useUi((store) => store.state);
  const recording = useUi((store) => store.recording);
  const openSettings = useUi((store) => store.openSettings);
  const closeSettings = useUi((store) => store.closeSettings);
  const update = useUi((store) => store.update);
  const setRecording = useUi((store) => store.setRecording);
  const onRecording = useCallback(
    (value: boolean) => {
      setRecording(value);
    },
    [setRecording],
  );
  const overrides = state?.settings.keybindings;
  const bindings = useMemo(
    () =>
      effectiveKeybindings(
        allCommands().map((command) => command.id),
        overrides ?? {},
      ),
    [overrides],
  );
  if (section === null || state === null) {
    return null;
  }

  return (
    <Dialog
      title={t('settings.title')}
      onClose={closeSettings}
      testId="settings-dialog"
      escapeDisabled={recording}
    >
      <div
        role="tablist"
        aria-label={t('settings.title')}
        className="mb-4 flex gap-1 border-b border-muted/40"
      >
        {sections.map((entry) => (
          <button
            key={entry.id}
            type="button"
            role="tab"
            id={`settings-tab-${entry.id}`}
            aria-selected={section === entry.id}
            aria-controls={`settings-panel-${entry.id}`}
            data-testid={`settings-tab-${entry.id}`}
            className={`text-ui -mb-px border-b-2 px-3 py-1.5 ${
              section === entry.id ? 'border-accent text-fg' : 'border-transparent text-muted hover:text-fg'
            }`}
            onClick={() => {
              openSettings(entry.id);
            }}
            onKeyDown={(event) => {
              const index = sections.findIndex((candidate) => candidate.id === section);
              const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
              if (step !== 0) {
                event.preventDefault();
                const next = sections[(index + step + sections.length) % sections.length];
                if (next) {
                  openSettings(next.id);
                  document.getElementById(`settings-tab-${next.id}`)?.focus();
                }
              }
            }}
          >
            {t(entry.key)}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={`settings-panel-${section}`} aria-labelledby={`settings-tab-${section}`}>
        {section === 'general' ? <GeneralSettings /> : null}
        {section === 'shortcuts' ? (
          <ShortcutSettings
            commands={allCommands()}
            bindings={bindings}
            overrides={state.settings.keybindings}
            platform={state.platform}
            onChange={(keybindings) => void update({ keybindings })}
            onRecording={onRecording}
          />
        ) : null}
        {section === 'library' ? <LibraryPanel /> : null}
      </div>
    </Dialog>
  );
}
