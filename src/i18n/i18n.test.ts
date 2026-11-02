import { describe, expect, it } from 'vitest';
import { detectLocale, dictionaries, isMessageKey, translate } from './index';

describe('dictionaries', () => {
  it('German and English define exactly the same keys', () => {
    expect(Object.keys(dictionaries.en).sort()).toEqual(Object.keys(dictionaries.de).sort());
  });

  it('contain no empty texts', () => {
    for (const dictionary of Object.values(dictionaries)) {
      for (const text of Object.values(dictionary)) {
        expect(text.trim()).not.toBe('');
      }
    }
  });

  it('use the same placeholders in both languages', () => {
    const placeholders = (text: string) => (text.match(/\{\w+\}/g) ?? []).sort();
    for (const key of Object.keys(dictionaries.de) as (keyof typeof dictionaries.de)[]) {
      expect(placeholders(dictionaries.en[key])).toEqual(placeholders(dictionaries.de[key]));
    }
  });
});

describe('detectLocale', () => {
  it('follows the system language list', () => {
    expect(detectLocale(['de-AT', 'en-US'])).toBe('de');
    expect(detectLocale(['fr-FR', 'en-GB'])).toBe('en');
    expect(detectLocale(['DE'])).toBe('de');
  });

  it('falls back to English', () => {
    expect(detectLocale(['fr-FR'])).toBe('en');
    expect(detectLocale([])).toBe('en');
  });
});

describe('translate', () => {
  it('replaces placeholders', () => {
    expect(translate('de', 'start.footer.version', { version: '0.1.0' })).toBe('Version 0.1.0');
  });

  it('keeps unknown placeholders visible', () => {
    expect(translate('en', 'start.footer.version')).toBe('Version {version}');
  });

  it('recognises message keys', () => {
    expect(isMessageKey('error.internal')).toBe(true);
    expect(isMessageKey('error.unknown')).toBe(false);
    expect(isMessageKey('toString')).toBe(false);
  });
});
