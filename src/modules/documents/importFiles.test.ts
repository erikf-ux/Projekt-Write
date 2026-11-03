import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { importFiles, MAX_IMPORT_BYTES } from './importFiles';

function summary(id: string, title: string) {
  return { id, title, format: 'md', sizeBytes: 1, createdAt: '', updatedAt: '', originalName: `${title}.md` };
}

describe('importFiles', () => {
  afterEach(() => {
    clearMocks();
  });

  it('asks about duplicates and follows the answer', async () => {
    const calls: { fileName: string; allowDuplicate: boolean; content: string }[] = [];
    mockIPC((command, args) => {
      if (command !== 'document_import') {
        throw new Error(command);
      }
      const request = (
        args as { request: { fileName: string; allowDuplicate: boolean; contentBase64: string } }
      ).request;
      calls.push({
        fileName: request.fileName,
        allowDuplicate: request.allowDuplicate,
        content: atob(request.contentBase64),
      });
      if (!request.allowDuplicate && request.fileName !== 'neu.md') {
        return { kind: 'duplicate', existing: summary('old', 'Alt') };
      }
      return { kind: 'imported', document: summary(request.fileName, request.fileName) };
    });
    const answers = { 'nochmal.md': 'import', 'gleich.md': 'existing', 'weg.md': 'skip' } as const;
    const ask = vi.fn((file: File) => Promise.resolve(answers[file.name as keyof typeof answers]));
    const files = ['neu.md', 'nochmal.md', 'gleich.md', 'weg.md'].map(
      (name) => new File([`# ${name}`], name),
    );
    const result = await importFiles(files, ask);
    expect(result.imported.map((document) => document.id)).toEqual(['neu.md', 'nochmal.md']);
    expect(result.reused.map((document) => document.id)).toEqual(['old']);
    expect(result.failed).toEqual([]);
    expect(ask).toHaveBeenCalledTimes(3);
    expect(calls.filter((call) => call.allowDuplicate).map((call) => call.fileName)).toEqual(['nochmal.md']);
    expect(calls[0]?.content).toBe('# neu.md');
  });

  it('reports refused files and files above the limit without sending them', async () => {
    const sent: string[] = [];
    mockIPC((_command, args) => {
      const request = (args as { request: { fileName: string } }).request;
      sent.push(request.fileName);
      throw { code: 'IMPORT_UNSUPPORTED_TYPE', messageKey: 'error.importUnsupportedType' };
    });
    const big = new File([new Uint8Array(MAX_IMPORT_BYTES + 1)], 'gross.txt');
    const result = await importFiles([big, new File(['x'], 'bild.png')], () => Promise.resolve('skip'));
    expect(result.failed.map((entry) => [entry.name, entry.error.code])).toEqual([
      ['gross.txt', 'DOCUMENT_TOO_LARGE'],
      ['bild.png', 'IMPORT_UNSUPPORTED_TYPE'],
    ]);
    expect(sent).toEqual(['bild.png']);
  });
});
