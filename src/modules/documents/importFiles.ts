/**
 * Import of files dropped onto the window (section 11: "Ablegen bedeutet
 * Import in die Bibliothek"; section 9). The webview hands over `File`
 * objects without a path; their content goes to Rust, which checks type
 * and content and stores a copy. The original file is only read.
 */
import { useStructure } from '@/modules/structure/structureStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';

/** Same limit as Rust (`MAX_IMPORT_BYTES`), checked before reading. */
export const MAX_IMPORT_BYTES = 10 * 1024 * 1024;

export type DuplicateAnswer = 'existing' | 'import' | 'skip';

export interface ImportSummary {
  imported: DocumentSummary[];
  /** Existing documents the user chose instead of a duplicate. */
  reused: DocumentSummary[];
  failed: { name: string; error: CommandError }[];
}

const TOO_LARGE: CommandError = { code: 'DOCUMENT_TOO_LARGE', messageKey: 'error.documentTooLarge' };

function readAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => {
      reject(reader.error ?? new Error('read failed'));
    };
    reader.onload = () => {
      const url = typeof reader.result === 'string' ? reader.result : '';
      resolve(url.slice(url.indexOf(',') + 1));
    };
    reader.readAsDataURL(file);
  });
}

/** Imports the files one after another. */
export async function importFiles(
  files: readonly File[],
  askDuplicate: (file: File, existing: DocumentSummary) => Promise<DuplicateAnswer>,
): Promise<ImportSummary> {
  const summary: ImportSummary = { imported: [], reused: [], failed: [] };
  for (const file of files) {
    if (file.size > MAX_IMPORT_BYTES) {
      summary.failed.push({ name: file.name, error: TOO_LARGE });
      continue;
    }
    let contentBase64: string;
    try {
      contentBase64 = await readAsBase64(file);
    } catch {
      summary.failed.push({
        name: file.name,
        error: { code: 'FILE_SYSTEM', messageKey: 'error.fileSystem' },
      });
      continue;
    }
    let result = await call('document_import', { fileName: file.name, contentBase64, allowDuplicate: false });
    if (result.ok && result.value.kind === 'duplicate') {
      const answer = await askDuplicate(file, result.value.existing);
      if (answer === 'existing') {
        summary.reused.push(result.value.existing);
        continue;
      }
      if (answer === 'skip') {
        continue;
      }
      result = await call('document_import', { fileName: file.name, contentBase64, allowDuplicate: true });
    }
    if (!result.ok) {
      summary.failed.push({ name: file.name, error: result.error });
    } else if (result.value.kind === 'imported') {
      summary.imported.push(result.value.document);
      // A .pwrite brings the tags of its envelope (ADR 0017).
      void useStructure.getState().refreshDocument(result.value.document.id);
    }
  }
  return summary;
}

/** Endings of text documents (Rust: `DOCUMENT_EXTENSIONS` of the media import). */
const DOCUMENT_EXTENSIONS = ['txt', 'md', 'markdown', 'pwrite'];

/**
 * Whether a dropped file is a text document (document import) rather than
 * a medium (media import, Phase 8). Rust checks the content either way.
 */
export function isDocumentFile(name: string): boolean {
  const dot = name.lastIndexOf('.');
  return dot > 0 && DOCUMENT_EXTENSIONS.includes(name.slice(dot + 1).toLowerCase());
}

/** Whether a drag carries files from outside (not a tab of this window). */
export function carriesFiles(dataTransfer: DataTransfer | null): boolean {
  return dataTransfer !== null && dataTransfer.types.includes('Files');
}
