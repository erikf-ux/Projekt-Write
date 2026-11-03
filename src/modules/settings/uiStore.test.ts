import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import type { UiState } from '@/types/generated/UiState';
import { applyThemeSetting, nextZoom, useUi } from './uiStore';

const state: UiState = {
  settings: {
    language: 'system',
    theme: 'system',
    zoom: 1,
    sidebar: { visible: true, width: 260 },
    limitLineWidth: true,
    keybindings: {},
  },
  locale: 'de',
  platform: 'windows',
  readOnly: false,
  customThemes: [],
};

describe('nextZoom', () => {
  it('walks the steps and stops at the limits', () => {
    expect(nextZoom(1, 1)).toBe(1.1);
    expect(nextZoom(1, -1)).toBe(0.9);
    expect(nextZoom(2, 1)).toBe(2);
    expect(nextZoom(0.5, -1)).toBe(0.5);
    expect(nextZoom(1.2, 1)).toBe(1.25);
  });
});

describe('applyThemeSetting', () => {
  it('uses imported themes and falls back to the system for unknown ones', () => {
    const colors = {
      surface: '#101820',
      sidebar: '#18222C',
      text: '#E6EDF3',
      textMuted: '#A9B4C0',
      accent: '#7FB2FF',
      error: '#FF9B8A',
      success: '#5FD38A',
      clusterBlue: '#7FB2FF',
      clusterGreen: '#5FD38A',
      clusterViolet: '#C3A0FF',
      clusterOrange: '#F5B060',
      clusterGrey: '#A9B0B8',
    };
    const id = '0123456789abcdef0123456789abcdef';
    applyThemeSetting(`custom:${id}`, [{ id, name: 'Nacht', base: 'dark', colors }]);
    expect(document.documentElement.dataset['theme']).toBe('custom');
    expect(document.getElementById('pw-theme-custom')?.textContent).toContain('--pw-color-surface: #101820;');
    applyThemeSetting('custom:ffffffffffffffffffffffffffffffff', []);
    expect(document.documentElement.dataset['theme']).toBe('light');
    applyThemeSetting('dark', []);
    expect(document.documentElement.dataset['theme']).toBe('dark');
  });
});

describe('update', () => {
  afterEach(() => {
    clearMocks();
    useUi.setState({ state: null, notice: null });
  });

  it('reverts a change Rust refuses and says so', async () => {
    mockIPC(() => {
      throw { code: 'SETTINGS_READ_ONLY', messageKey: 'error.settingsReadOnly' };
    });
    useUi.setState({ state });
    expect(await useUi.getState().update({ zoom: 1.5 })).toBe(false);
    expect(useUi.getState().state?.settings.zoom).toBe(1);
    expect(useUi.getState().notice?.error?.code).toBe('SETTINGS_READ_ONLY');
  });

  it('takes the state Rust returns', async () => {
    mockIPC((command, args) => {
      expect(command).toBe('ui_update_settings');
      const request = (args as { request: { settings: UiState['settings'] } }).request;
      return { ...state, locale: 'en', settings: request.settings };
    });
    useUi.setState({ state });
    expect(await useUi.getState().update({ language: 'en' })).toBe(true);
    expect(useUi.getState().state?.locale).toBe('en');
  });
});
