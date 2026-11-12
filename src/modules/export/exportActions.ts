/**
 * Export, print and packages in the interface (Phase 10, section 21, ADR
 * 0021). The editor content is converted here (the schema lives in the
 * frontend); Rust shows the dialogs and writes the files.
 *
 * * TXT and Markdown: what the conversion would lose is asked first, like
 *   converting a document (section 10, "Verlustschutz").
 * * Printing and PDF: print CSS and `window.print`, the system dialog.
 * * Library export: Markdown and text are written by Rust at once; every
 *   `.pwrite` document is read, converted and sent one by one.
 */
import { ask } from '@/components/prompt/promptStore';
import { translate, useLocale } from '@/i18n';
import { activeDocument, conversionDetails, useWorkspace } from '@/modules/documents/workspaceStore';
import { convertContent, needsConfirmation, type ConversionSource } from '@/modules/editor/convert';
import { panelElementId } from '@/modules/editor/DocumentEditor';
import { getSession } from '@/modules/editor/session';
import { useMedia } from '@/modules/media/mediaStore';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';

const t = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate(useLocale.getState().locale, key, params);

/** The active library document with its editor, saved first. */
async function activeLibrary() {
  const workspace = useWorkspace.getState();
  const document = activeDocument(workspace);
  if (!document?.loaded || document.external) {
    return null;
  }
  const session = getSession(document.id);
  if (!session) {
    return null;
  }
  await workspace.flush(document.id);
  return { document, session };
}

/** The content of a tab in `to`; `null` when the user declines a loss. */
async function contentAs(
  from: DocumentFormat,
  to: DocumentFormat,
  source: ConversionSource,
): Promise<string | null> {
  if (from === to) {
    if (source.kind === 'text') {
      return source.text;
    }
    // Rich text in its own format: the conversion to it loses nothing.
    return convertContent(from === 'md' ? 'pwrite' : from, to, source).text;
  }
  const conversion = convertContent(from, to, source);
  if (needsConfirmation(conversion)) {
    const answer = await ask({
      titleKey: 'export.lossTitle',
      textKey: 'export.lossWarning',
      params: { details: conversionDetails(conversion) },
      choices: [
        { id: 'export', labelKey: 'export.anyway', tone: 'primary' },
        { id: 'cancel', labelKey: 'convert.cancel' },
      ],
      cancel: 'cancel',
      testId: 'export-loss-prompt',
    });
    if (answer !== 'export') {
      return null;
    }
  }
  return conversion.text;
}

function notifyError(error: CommandError) {
  useUi.getState().showNotice({ tone: 'error', key: 'export.failed', error });
}

/** "Exportieren als TXT …" / "Exportieren als Markdown …". */
export async function exportActive(format: 'txt' | 'markdown'): Promise<boolean> {
  const active = await activeLibrary();
  if (!active) {
    return false;
  }
  const { document, session } = active;
  const text = await contentAs(document.format, format === 'txt' ? 'txt' : 'md', session.conversionSource());
  if (text === null) {
    return false;
  }
  const result = await call('export_document', {
    id: document.id,
    format,
    text,
    title: t(format === 'txt' ? 'command.export.txt' : 'command.export.markdown'),
  });
  if (!result.ok) {
    notifyError(result.error);
    return false;
  }
  if (result.value) {
    useUi.getState().showNotice({
      tone: 'info',
      key: result.value.media > 0 ? 'export.doneWithMedia' : 'export.done',
      params: { location: result.value.location, media: result.value.media },
    });
  }
  return result.value !== null;
}

/** "Als Paket exportieren …": the document as `.pwrite` with its media. */
export async function exportPackActive(): Promise<boolean> {
  const active = await activeLibrary();
  if (!active) {
    return false;
  }
  const { document, session } = active;
  const content = await contentAs(document.format, 'pwrite', session.conversionSource());
  if (content === null) {
    return false;
  }
  const summary = useWorkspace.getState().documents.find((entry) => entry.id === document.id);
  const envelope = {
    format: 'pwrite',
    formatVersion: 1,
    id: document.id,
    title: document.title,
    createdAt: summary?.createdAt ?? new Date().toISOString(),
    updatedAt: summary?.updatedAt ?? new Date().toISOString(),
    language: useLocale.getState().locale,
    tags: [],
    content: JSON.parse(content) as unknown,
    media: [],
  };
  const result = await call('export_pack', {
    id: document.id,
    document: JSON.stringify(envelope),
    title: t('command.export.pack'),
  });
  if (!result.ok) {
    notifyError(result.error);
    return false;
  }
  if (result.value !== null) {
    useUi.getState().showNotice({ tone: 'info', key: 'export.packDone', params: { media: result.value } });
  }
  return result.value !== null;
}

/** "Paket importieren …": a `.pwritepack` becomes a new document. */
export async function importPack(): Promise<boolean> {
  const result = await call('import_pack', { title: t('command.import.pack') });
  if (!result.ok) {
    useUi.getState().showNotice({ tone: 'error', key: 'export.packRefused', error: result.error });
    return false;
  }
  if (!result.value) {
    return false;
  }
  const { documentId, imported, reused } = result.value;
  await useWorkspace.getState().refresh();
  await useMedia.getState().load();
  await useWorkspace.getState().openDocument(documentId);
  useUi.getState().showNotice({ tone: 'info', key: 'export.packImported', params: { imported, reused } });
  return true;
}

/** Progress of a library export, for the notice. */
export interface LibraryExportProgress {
  done: number;
  total: number;
}

/** "Bibliothek exportieren …" (section 21: usable without the app). */
export async function exportLibrary(
  onProgress?: (progress: LibraryExportProgress) => void,
): Promise<boolean> {
  await useWorkspace.getState().flushAll();
  const begun = await call('export_library_begin', { title: t('command.export.library') });
  if (!begun.ok) {
    notifyError(begun.error);
    return false;
  }
  if (!begun.value) {
    return false;
  }
  const { exportId, toConvert, written } = begun.value;
  const total = written + toConvert.length;
  let done = written;
  onProgress?.({ done, total });
  for (const item of toConvert) {
    const read = await call('document_read', { id: item.id });
    if (!read.ok) {
      await call('export_library_cancel', { exportId });
      notifyError(read.error);
      return false;
    }
    const markdown = convertContent('pwrite', 'md', { kind: 'text', text: read.value.text }).text;
    const added = await call('export_library_add', { exportId, id: item.id, markdown });
    if (!added.ok) {
      await call('export_library_cancel', { exportId });
      notifyError(added.error);
      return false;
    }
    done += 1;
    onProgress?.({ done, total });
  }
  const finished = await call('export_library_finish', { exportId });
  if (!finished.ok) {
    notifyError(finished.error);
    return false;
  }
  useUi.getState().showNotice({
    tone: 'info',
    key: 'export.libraryDone',
    params: {
      location: finished.value.location,
      documents: finished.value.documents,
      media: finished.value.media,
    },
  });
  return true;
}

/** The container the print CSS shows (index.css). */
export const PRINT_ROOT_ID = 'pw-print';

/**
 * "Drucken …" and "Als PDF exportieren …": the system dialog over print
 * CSS (section 21). A copy of the writing surface of the active document,
 * with its title, is the only thing printed.
 */
export async function printActive(asPdf: boolean): Promise<boolean> {
  const workspace = useWorkspace.getState();
  const document = activeDocument(workspace);
  if (!document?.loaded) {
    return false;
  }
  await workspace.flush(document.id);
  const panel = window.document.getElementById(panelElementId(document.id));
  const surface = [...(panel?.querySelectorAll<HTMLElement>('[data-testid="editor"]') ?? [])].find(
    (element) => element.offsetParent !== null || element.getClientRects().length > 0,
  );
  if (!surface) {
    return false;
  }
  if (asPdf) {
    useUi.getState().showNotice({ tone: 'info', key: 'export.pdfHint' });
  }
  window.document.getElementById(PRINT_ROOT_ID)?.remove();
  const root = window.document.createElement('div');
  root.id = PRINT_ROOT_ID;
  root.setAttribute('data-testid', 'print-root');
  const title = window.document.createElement('p');
  title.className = 'pw-print-title';
  title.textContent = document.title;
  const copy = surface.cloneNode(true) as HTMLElement;
  copy.removeAttribute('contenteditable');
  for (const editable of copy.querySelectorAll('[contenteditable]')) {
    editable.removeAttribute('contenteditable');
  }
  root.append(title, copy);
  window.document.body.append(root);
  const previousTitle = window.document.title;
  // The title of the window becomes the suggested name of a PDF.
  window.document.title = document.title;
  window.document.documentElement.setAttribute('data-printing', '');
  // Not every webview blocks in print(): clean up when printing is over.
  const cleanUp = () => {
    window.removeEventListener('afterprint', cleanUp);
    window.document.documentElement.removeAttribute('data-printing');
    window.document.title = previousTitle;
    root.remove();
  };
  window.addEventListener('afterprint', cleanUp);
  try {
    window.print();
  } catch {
    cleanUp();
    return false;
  }
  return true;
}
