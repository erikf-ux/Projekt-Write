/**
 * Interface state: the settings from `settings.json` (through Rust), the
 * language, the theme, focus mode, and which overlay is open.
 */
import { create } from 'zustand';
import { useLocale, type MessageKey } from '@/i18n';
import { applyCustomTheme, applyTheme, followSystemTheme } from '@/modules/themes/applyTheme';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { CustomTheme } from '@/types/generated/CustomTheme';
import type { UiSettings } from '@/types/generated/UiSettings';
import type { UiState } from '@/types/generated/UiState';

/** Zoom steps of the interface (section 11: "Oberflächenzoom"). */
export const ZOOM_STEPS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2] as const;
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 2;

export type SettingsSection = 'general' | 'shortcuts' | 'library';

/** A short message at the bottom of the window. */
export interface Notice {
  tone: 'info' | 'error';
  key: MessageKey;
  params?: Record<string, string | number>;
  error?: CommandError;
}

interface UiStore {
  state: UiState | null;
  loadError: CommandError | null;
  focusMode: boolean;
  paletteOpen: boolean;
  settingsSection: SettingsSection | null;
  /** The shortcut recorder has the keyboard. */
  recording: boolean;
  notice: Notice | null;
  /** Quitting although a save failed: ask first. */
  closePrompt: boolean;
  load: () => Promise<void>;
  /** Changes some settings; returns whether they were saved. */
  update: (change: Partial<UiSettings>) => Promise<boolean>;
  applyState: (state: UiState) => void;
  setFocusMode: (enabled: boolean) => Promise<void>;
  setPaletteOpen: (open: boolean) => void;
  openSettings: (section: SettingsSection) => void;
  closeSettings: () => void;
  setRecording: (recording: boolean) => void;
  showNotice: (notice: Notice | null) => void;
}

let stopFollowingSystem: (() => void) | null = null;

/** Applies the theme setting; an unknown imported theme falls back to the system. */
export function applyThemeSetting(theme: string, customThemes: readonly CustomTheme[]): void {
  stopFollowingSystem?.();
  stopFollowingSystem = null;
  if (theme === 'light' || theme === 'dark') {
    applyTheme(theme);
    return;
  }
  const custom = theme.startsWith('custom:')
    ? customThemes.find((t) => `custom:${t.id}` === theme)
    : undefined;
  if (custom) {
    applyCustomTheme({ base: custom.base, colors: custom.colors });
    return;
  }
  stopFollowingSystem = followSystemTheme();
}

export function nextZoom(current: number, direction: 1 | -1): number {
  if (direction === 1) {
    return ZOOM_STEPS.find((step) => step > current + 1e-9) ?? MAX_ZOOM;
  }
  return [...ZOOM_STEPS].reverse().find((step) => step < current - 1e-9) ?? MIN_ZOOM;
}

export const useUi = create<UiStore>((set, get) => ({
  state: null,
  loadError: null,
  focusMode: false,
  paletteOpen: false,
  settingsSection: null,
  recording: false,
  notice: null,
  closePrompt: false,

  load: async () => {
    const result = await call('ui_state', {});
    if (result.ok) {
      get().applyState(result.value);
    } else {
      set({ loadError: result.error });
    }
  },

  applyState: (state) => {
    set({ state, loadError: null });
    useLocale.getState().setLocale(state.locale);
    applyThemeSetting(state.settings.theme, state.customThemes);
  },

  update: async (change) => {
    const current = get().state;
    if (!current) {
      return false;
    }
    const settings: UiSettings = { ...current.settings, ...change };
    // Apply at once; revert if Rust refuses.
    get().applyState({ ...current, settings });
    const result = await call('ui_update_settings', { settings });
    if (result.ok) {
      get().applyState(result.value);
      return true;
    }
    get().applyState(current);
    set({ notice: { tone: 'error', key: 'notice.settingsNotSaved', error: result.error } });
    return false;
  },

  setFocusMode: async (enabled) => {
    set({ focusMode: enabled });
    await call('ui_set_focus_mode', { enabled });
  },

  setPaletteOpen: (open) => {
    set({ paletteOpen: open });
  },
  openSettings: (section) => {
    set({ settingsSection: section, paletteOpen: false });
  },
  closeSettings: () => {
    set({ settingsSection: null, recording: false });
  },
  setRecording: (recording) => {
    set({ recording });
  },
  showNotice: (notice) => {
    set({ notice });
  },
}));
