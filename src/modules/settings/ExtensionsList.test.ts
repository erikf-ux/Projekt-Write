import { describe, expect, it } from 'vitest';
import type { ExtensionInfo } from '@/types/generated/ExtensionInfo';
import { groupExtensions } from './ExtensionsList';

const core: ExtensionInfo[] = [
  { registry: 'documentFormats', id: 'txt', origin: 'builtin', addOn: false },
  { registry: 'documentFormats', id: 'md', origin: 'builtin', addOn: false },
  { registry: 'documentFormats', id: 'pwrite', origin: 'builtin', addOn: false },
  { registry: 'exporters', id: 'txt', origin: 'builtin', addOn: false },
  { registry: 'languageEngines', id: 'spelling', origin: 'builtin', addOn: false },
  { registry: 'languageEngines', id: 'languagetool', origin: 'builtin', addOn: true },
];

describe('the list of built-in extensions', () => {
  it('names every registry of Rust and of the interface, each entry once', () => {
    const groups = new Map(groupExtensions(core).map((group) => [group.key, group.entries]));
    expect([...groups.keys()]).toEqual([
      'editorFeatures',
      'documentFormats',
      'languageEngines',
      'exporters',
      'meshRenderers',
    ]);
    expect(groups.get('documentFormats')?.map((entry) => entry.id)).toEqual(['txt', 'md', 'pwrite']);
    expect(groups.get('languageEngines')).toEqual([
      { id: 'spelling', addOn: false },
      { id: 'languagetool', addOn: true },
    ]);
    // The interface adds printing and PDF to the exporters of Rust.
    const exporters = groups.get('exporters')?.map((entry) => entry.id) ?? [];
    expect(exporters[0]).toBe('txt');
    expect(exporters).toEqual(expect.arrayContaining(['print', 'pdf']));
    expect(new Set(exporters).size).toBe(exporters.length);
    expect(groups.get('meshRenderers')?.map((entry) => entry.id)).toEqual(['react-flow']);
    expect(groups.get('editorFeatures')?.length).toBeGreaterThan(5);
  });
});
