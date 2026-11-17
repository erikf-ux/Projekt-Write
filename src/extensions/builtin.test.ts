import { getSchema, type AnyExtension } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import { RegistrySealedError } from '@/core/registry';
import { createAppCommands } from '@/app/commands';
import { editorExtensions, schemaExtensions } from '@/modules/editor/features';
import { formatAdapter, markdownFormat } from '@/modules/editor/formats';
import { markdownExtensions } from '@/modules/editor/schema/extensions';
import { MESH_RENDERER } from '@/modules/mesh/renderer';
import { registerBuiltins } from './builtin';
import { createAppRegistries, registries, registryList, sealRegistries } from './registries';

const names = (extensions: AnyExtension[]) => extensions.map((extension) => extension.name);

/** The Markdown schema as `markdownExtensions()` listed it before the registries. */
const MARKDOWN_BEFORE = [
  'starterKit',
  'doc',
  'paragraph',
  'heading',
  'hardBreak',
  'code',
  'codeBlock',
  'bulletList',
  'orderedList',
  'listItem',
  'link',
  'linkedImages',
  'entities',
  'taskList',
  'taskItem',
  'tableKit',
  'table',
  'image',
  'rawBlock',
  'rawInline',
  'footnotes',
  'lang',
];

/** The minimal schema as `plainExtensions()` listed it before the registries. */
const PLAIN_BEFORE = ['doc', 'line', 'text', 'undoRedo'];

describe('built-in extensions', () => {
  it('fill fresh registries and can be sealed', () => {
    const fresh = createAppRegistries();
    registerBuiltins(fresh);
    expect(fresh.editorFeatures.list().map((feature) => feature.id)).toEqual([
      'core-text',
      'lists',
      'links',
      'entities',
      'task-lists',
      'tables',
      'images',
      'raw-markdown',
      'language-parts',
      'plain-text',
      'find',
      'language-check',
    ]);
    expect(fresh.documentFormats.list().map((format) => format.id)).toEqual(['txt', 'md', 'pwrite']);
    expect(fresh.exporters.list().map((exporter) => exporter.id)).toEqual([
      'print',
      'pdf',
      'txt',
      'markdown',
      'pack',
      'library',
    ]);
    expect(fresh.meshRenderers.list().map((renderer) => renderer.id)).toEqual([MESH_RENDERER]);
    expect(registryList(fresh).some((registry) => registry.sealed)).toBe(false);
    sealRegistries(fresh);
    expect(registryList(fresh).every((registry) => registry.sealed)).toBe(true);
  });

  it('cannot be registered twice', () => {
    const fresh = createAppRegistries();
    registerBuiltins(fresh);
    expect(() => {
      registerBuiltins(fresh);
    }).toThrow(/registered twice/);
  });

  it('leave the app registries sealed: nothing can be registered after start', () => {
    // src/test/setup.ts starts the registries like main.tsx does.
    for (const registry of registryList(registries)) {
      expect(registry.sealed).toBe(true);
    }
    expect(() => {
      registries.exporters.register({ ...registries.exporters.require('txt'), id: 'foreign' });
    }).toThrow(RegistrySealedError);
    expect(() => {
      registries.editorFeatures.register({
        id: 'foreign',
        schemas: ['markdown'],
        editorOnly: true,
        extensions: () => [],
      });
    }).toThrow(RegistrySealedError);
    expect(() => {
      registries.documentFormats.register({ ...markdownFormat });
    }).toThrow(RegistrySealedError);
    expect(() => {
      registries.meshRenderers.register({ id: 'foreign', component: () => null });
    }).toThrow(RegistrySealedError);
    expect(registries.exporters.get('foreign')).toBeUndefined();
  });

  it('are not reachable from window', () => {
    const reachable = Object.getOwnPropertyNames(window).filter((name) => {
      try {
        const value: unknown = (window as unknown as Record<string, unknown>)[name];
        return typeof value === 'object' && value !== null && 'register' in value && 'seal' in value;
      } catch {
        return false;
      }
    });
    expect(reachable).toEqual([]);
  });
});

describe('editor features', () => {
  it('give the Markdown schema the same extensions in the same order as before', () => {
    expect(names(markdownExtensions())).toEqual(MARKDOWN_BEFORE);
    expect(names(schemaExtensions('markdown'))).toEqual(MARKDOWN_BEFORE);
    const schema = getSchema(markdownExtensions());
    expect(Object.keys(schema.nodes)).toEqual([
      'paragraph',
      'blockquote',
      'horizontalRule',
      'text',
      'doc',
      'heading',
      'hardBreak',
      'codeBlock',
      'bulletList',
      'orderedList',
      'listItem',
      'taskList',
      'taskItem',
      'tableCell',
      'tableHeader',
      'tableRow',
      'table',
      'image',
      'rawBlock',
      'rawInline',
    ]);
    expect(Object.keys(schema.marks)).toEqual([
      'link',
      'bold',
      'italic',
      'strike',
      'underline',
      'code',
      'lang',
    ]);
  });

  it('give the editors the schema, then find, then the language check', () => {
    const rich = editorExtensions('markdown', { documentId: 'doc-1', source: false });
    expect(names(rich)).toEqual([...MARKDOWN_BEFORE, 'findAndReplace', 'languageCheck']);
    expect(names(schemaExtensions('plain'))).toEqual(PLAIN_BEFORE);
    const plain = editorExtensions('plain', { documentId: null, source: false });
    expect(names(plain)).toEqual([...PLAIN_BEFORE, 'findAndReplace', 'languageCheck']);
  });

  it('switch the language check off in the Markdown source view only', () => {
    const check = (source: boolean) =>
      editorExtensions('plain', { documentId: 'doc-1', source }).find(
        (extension) => extension.name === 'languageCheck',
      )?.options as { documentId: string | null; enabled: boolean } | undefined;
    expect(check(false)).toEqual({ documentId: 'doc-1', enabled: true });
    expect(check(true)).toEqual({ documentId: 'doc-1', enabled: false });
  });
});

describe('document formats', () => {
  it('have an adapter for every format', () => {
    expect(formatAdapter('md').sourceView).toBe(true);
    expect(formatAdapter('pwrite').sourceView).toBe(false);
    expect(formatAdapter('txt').open('a\tb')).toEqual({ view: 'source', text: 'a\tb', issues: [] });
    expect(formatAdapter('md').open('# Titel\n')).toMatchObject({ view: 'rich', issues: [] });
    expect(formatAdapter('pwrite').open('kein JSON')).toMatchObject({
      view: 'rich',
      issues: [{ kind: 'invalid' }],
    });
  });
});

describe('exporters', () => {
  it('give the export commands the same ids, titles and order as before', () => {
    const commands = createAppCommands();
    const start = commands.findIndex((command) => command.id === 'document.print');
    expect(commands[start - 1]?.id).toBe('document.takeIntoLibrary');
    expect(
      commands.slice(start, start + 8).map((command) => [command.id, command.titleKey, command.category]),
    ).toEqual([
      ['document.print', 'command.document.print', 'document'],
      ['export.pdf', 'command.export.pdf', 'document'],
      ['export.txt', 'command.export.txt', 'document'],
      ['export.markdown', 'command.export.markdown', 'document'],
      ['export.pack', 'command.export.pack', 'document'],
      ['import.pack', 'command.import.pack', 'document'],
      ['export.library', 'command.export.library', 'app'],
      ['backup.create', 'command.backup.create', 'app'],
    ]);
  });
});
