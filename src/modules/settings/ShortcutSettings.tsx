/**
 * Shortcut settings (section 11: "Konfigurierbare Tastenkürzel mit
 * Konflikterkennung"). Every command can get another shortcut, lose its
 * shortcut or return to the default. A shortcut that another command
 * already uses is reported before it is assigned; conflicts in
 * `settings.json` (edited by hand) are listed at the top.
 */
import { TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useLocale, useT, type MessageKey } from '@/i18n';
import {
  chordFromKey,
  defaultKeybindings,
  findConflicts,
  formatChord,
  isCanonicalChord,
  reservedChords,
} from '@/modules/commands/keymap';
import type { CommandCategory, CommandDefinition, CommandId } from '@/modules/commands/types';
import type { Platform } from '@/types/generated/Platform';

const SMALL_BUTTON =
  'text-ui inline-flex h-6 items-center rounded-pw border border-muted px-2 text-fg disabled:opacity-60';

const categoryKeys: Record<CommandCategory, MessageKey> = {
  document: 'category.document',
  tab: 'category.tab',
  view: 'category.view',
  app: 'category.app',
};

interface ShortcutSettingsProps {
  commands: readonly CommandDefinition[];
  bindings: ReadonlyMap<CommandId, readonly string[]>;
  overrides: Readonly<Record<string, string[]>>;
  platform: Platform;
  onChange: (overrides: Record<string, string[]>) => void;
  onRecording: (recording: boolean) => void;
}

type Recording =
  | { command: CommandId; step: 'listen'; message?: MessageKey }
  | { command: CommandId; step: 'conflict'; chord: string; others: CommandId[] };

export function ShortcutSettings({
  commands,
  bindings,
  overrides,
  platform,
  onChange,
  onRecording,
}: ShortcutSettingsProps) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const [filter, setFilter] = useState('');
  const [recording, setRecording] = useState<Recording | null>(null);
  const recorder = useRef<HTMLDivElement>(null);
  const conflicts = useMemo(() => findConflicts(bindings), [bindings]);
  const titleOf = (id: CommandId) => {
    const command = commands.find((entry) => entry.id === id);
    return command ? t(command.titleKey) : id;
  };
  const show = (chord: string) => formatChord(chord, locale, platform);

  useEffect(() => {
    onRecording(recording !== null);
    if (recording?.step === 'listen') {
      recorder.current?.focus();
    } else if (recording?.step === 'conflict') {
      document.querySelector<HTMLElement>('[data-testid="shortcut-conflict-question"] button')?.focus();
    }
  }, [recording, onRecording]);
  useEffect(
    () => () => {
      onRecording(false);
    },
    [onRecording],
  );

  const assign = (command: CommandId, chord: string, replace: CommandId[]) => {
    const next = { ...overrides, [command]: [chord] };
    for (const other of replace) {
      next[other] = (bindings.get(other) ?? []).filter((existing) => existing !== chord);
    }
    onChange(next);
    setRecording(null);
  };

  const onRecordKey = (event: KeyboardEvent, command: CommandId) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape' && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      setRecording(null);
      return;
    }
    const chord = chordFromKey(
      {
        key: event.key,
        ctrlKey: event.ctrlKey,
        altKey: event.altKey,
        shiftKey: event.shiftKey,
        metaKey: event.metaKey,
        altGraph: event.getModifierState('AltGraph'),
      },
      platform,
    );
    if (chord === null) {
      // Modifier keys alone: keep listening. Plain characters: explain.
      if (!['Control', 'Shift', 'Alt', 'Meta', 'AltGraph'].includes(event.key)) {
        setRecording({ command, step: 'listen', message: 'shortcuts.needsModifier' });
      }
      return;
    }
    if (!isCanonicalChord(chord) || reservedChords.has(chord)) {
      setRecording({ command, step: 'listen', message: 'shortcuts.reserved' });
      return;
    }
    const others = [...bindings]
      .filter(([id, chords]) => id !== command && chords.includes(chord))
      .map(([id]) => id);
    if (others.length > 0) {
      setRecording({ command, step: 'conflict', chord, others });
      return;
    }
    assign(command, chord, []);
  };

  const words = filter.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const rows = commands
    .map((command) => ({ command, title: t(command.titleKey), category: t(categoryKeys[command.category]) }))
    .filter(({ title, category }) =>
      words.every((word) => `${title} ${category}`.toLocaleLowerCase().includes(word)),
    )
    .sort((a, b) => a.category.localeCompare(b.category, locale) || a.title.localeCompare(b.title, locale));

  return (
    <div data-testid="settings-shortcuts">
      {conflicts.length > 0 ? (
        <div
          role="alert"
          className="text-ui mb-3 rounded-pw border border-danger p-3"
          data-testid="shortcut-conflicts"
        >
          <p className="flex items-center gap-2 font-semibold text-danger">
            <TriangleAlert aria-hidden="true" className="size-4" />
            {t('shortcuts.conflictsTitle')}
          </p>
          <ul className="mt-1">
            {conflicts.map((conflict) => (
              <li key={conflict.chord}>
                {t('shortcuts.conflict', {
                  chord: show(conflict.chord),
                  commands: conflict.commands.map(titleOf).join(', '),
                })}
              </li>
            ))}
          </ul>
          <p className="mt-1 text-muted">{t('shortcuts.conflictHint')}</p>
        </div>
      ) : null}
      <label className="text-ui mb-3 flex items-center gap-2">
        <span className="text-muted">{t('shortcuts.search')}</span>
        <input
          type="search"
          data-autofocus
          className="text-ui h-7 flex-1 rounded-pw border border-muted bg-surface px-2 text-fg"
          value={filter}
          onChange={(event) => {
            setFilter(event.target.value);
          }}
        />
      </label>
      <table className="text-ui w-full border-collapse">
        <thead>
          <tr className="text-left text-muted">
            <th className="py-1 font-normal">{t('shortcuts.command')}</th>
            <th className="py-1 font-normal">{t('shortcuts.keys')}</th>
            <th className="py-1 font-normal">
              <span className="sr-only">{t('shortcuts.actions')}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ command, title, category }) => {
            const chords = bindings.get(command.id) ?? [];
            const changed = overrides[command.id] !== undefined;
            const isRecording = recording?.command === command.id;
            return (
              <tr
                key={command.id}
                className="border-t border-muted/30 align-top"
                data-shortcut-row={command.id}
              >
                <td className="py-1.5 pr-2">
                  <span className="text-muted">{category}: </span>
                  {title}
                </td>
                <td className="py-1.5 pr-2">
                  {isRecording && recording.step === 'listen' ? (
                    <div
                      ref={recorder}
                      tabIndex={0}
                      role="textbox"
                      aria-label={t('shortcuts.recordLabel', { command: title })}
                      data-testid="shortcut-recorder"
                      className="rounded-pw border border-accent px-2 py-0.5 text-fg"
                      onKeyDown={(event) => {
                        onRecordKey(event, command.id);
                      }}
                    >
                      {t('shortcuts.recording')}
                      {recording.message ? (
                        <span className="block text-danger">{t(recording.message)}</span>
                      ) : null}
                    </div>
                  ) : isRecording && recording.step === 'conflict' ? (
                    <div role="alert" className="text-danger" data-testid="shortcut-conflict-question">
                      {t('shortcuts.taken', {
                        chord: show(recording.chord),
                        commands: recording.others.map(titleOf).join(', '),
                      })}
                      <div className="mt-1 flex gap-2">
                        <button
                          type="button"
                          className={SMALL_BUTTON}
                          data-autofocus
                          onClick={() => {
                            assign(command.id, recording.chord, recording.others);
                          }}
                        >
                          {t('shortcuts.replace')}
                        </button>
                        <button
                          type="button"
                          className={SMALL_BUTTON}
                          onClick={() => {
                            setRecording(null);
                          }}
                        >
                          {t('shortcuts.cancel')}
                        </button>
                      </div>
                    </div>
                  ) : chords.length > 0 ? (
                    <span className="flex flex-wrap gap-1">
                      {chords.map((chord) => {
                        const inConflict = conflicts.some((conflict) => conflict.chord === chord);
                        return (
                          <kbd
                            key={chord}
                            className={`rounded-pw border px-1.5 font-sans ${inConflict ? 'border-danger text-danger' : 'border-muted'}`}
                          >
                            {show(chord)}
                            {inConflict ? (
                              <span className="sr-only"> ({t('shortcuts.inConflict')})</span>
                            ) : null}
                          </kbd>
                        );
                      })}
                    </span>
                  ) : (
                    <span className="text-muted">{t('shortcuts.none')}</span>
                  )}
                </td>
                <td className="py-1.5 text-right whitespace-nowrap">
                  <span className="inline-flex gap-1">
                    <button
                      type="button"
                      className={SMALL_BUTTON}
                      aria-label={t('shortcuts.changeLabel', { command: title })}
                      data-testid={`shortcut-change-${command.id}`}
                      onClick={() => {
                        setRecording({ command: command.id, step: 'listen' });
                      }}
                    >
                      {t('shortcuts.change')}
                    </button>
                    <button
                      type="button"
                      className={SMALL_BUTTON}
                      disabled={chords.length === 0}
                      aria-label={t('shortcuts.removeLabel', { command: title })}
                      onClick={() => {
                        onChange({ ...overrides, [command.id]: [] });
                      }}
                    >
                      {t('shortcuts.remove')}
                    </button>
                    <button
                      type="button"
                      className={SMALL_BUTTON}
                      disabled={!changed}
                      aria-label={t('shortcuts.resetLabel', { command: title })}
                      title={
                        (defaultKeybindings[command.id] ?? []).map(show).join(', ') || t('shortcuts.none')
                      }
                      onClick={() => {
                        onChange(
                          Object.fromEntries(Object.entries(overrides).filter(([id]) => id !== command.id)),
                        );
                      }}
                    >
                      {t('shortcuts.reset')}
                    </button>
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
