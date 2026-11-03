/**
 * Drag and drop of files onto the window (section 11). Works with
 * `dragDropEnabled: false` (HTML5 drag and drop, needed on Windows).
 * Folders are refused; files are imported (see `importFiles`).
 */
import { FileDown } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useT } from '@/i18n';
import { useUi } from '@/modules/settings/uiStore';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { carriesFiles, importFiles, type DuplicateAnswer } from './importFiles';
import { useWorkspace } from './workspaceStore';

interface Question {
  file: string;
  existing: DocumentSummary;
  answer: (answer: DuplicateAnswer) => void;
}

export function DropZone({ children }: { children: ReactNode }) {
  const t = useT();
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);
  const [question, setQuestion] = useState<Question | null>(null);
  const depth = useRef(0);

  const run = useCallback(
    async (files: File[]) => {
      if (files.length === 0) {
        return;
      }
      setBusy(true);
      const summary = await importFiles(
        files,
        (file, existing) =>
          new Promise<DuplicateAnswer>((resolve) => {
            setQuestion({
              file: file.name,
              existing,
              answer: (answer) => {
                setQuestion(null);
                resolve(answer);
              },
            });
          }),
      );
      setBusy(false);
      const workspace = useWorkspace.getState();
      for (const document of summary.imported) {
        workspace.addDocument(document);
      }
      const first = summary.imported[0] ?? summary.reused[0];
      if (first) {
        await workspace.openDocument(first.id);
      }
      const ui = useUi.getState();
      const failure = summary.failed[0];
      if (failure) {
        ui.showNotice({
          tone: 'error',
          key: 'import.failed',
          params: {
            names: summary.failed.map((entry) => entry.name).join(', '),
            reason: errorText(t, failure.error),
            count: summary.imported.length,
          },
        });
      } else if (summary.imported.length > 0) {
        ui.showNotice({ tone: 'info', key: 'import.done', params: { count: summary.imported.length } });
      }
    },
    [t],
  );

  useEffect(() => {
    const onDragEnter = (event: DragEvent) => {
      if (carriesFiles(event.dataTransfer)) {
        depth.current += 1;
        setOver(true);
      }
    };
    const onDragLeave = (event: DragEvent) => {
      if (carriesFiles(event.dataTransfer)) {
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) {
          setOver(false);
        }
      }
    };
    const onDragOver = (event: DragEvent) => {
      if (carriesFiles(event.dataTransfer) && event.dataTransfer) {
        // Without this the webview would open the file itself.
        event.preventDefault();
        event.dataTransfer.dropEffect = busy ? 'none' : 'copy';
      }
    };
    const onDrop = (event: DragEvent) => {
      if (!carriesFiles(event.dataTransfer) || !event.dataTransfer) {
        return;
      }
      event.preventDefault();
      depth.current = 0;
      setOver(false);
      if (busy) {
        return;
      }
      const items = [...event.dataTransfer.items];
      const folder = items.some((item) => item.webkitGetAsEntry()?.isDirectory === true);
      const files = [...event.dataTransfer.files];
      if (folder) {
        useUi.getState().showNotice({ tone: 'error', key: 'import.noFolders' });
        return;
      }
      void run(files);
    };
    window.addEventListener('dragenter', onDragEnter);
    window.addEventListener('dragleave', onDragLeave);
    window.addEventListener('dragover', onDragOver);
    window.addEventListener('drop', onDrop);
    return () => {
      window.removeEventListener('dragenter', onDragEnter);
      window.removeEventListener('dragleave', onDragLeave);
      window.removeEventListener('dragover', onDragOver);
      window.removeEventListener('drop', onDrop);
    };
  }, [busy, run]);

  return (
    <>
      {children}
      {over ? (
        <div
          className="pointer-events-none fixed inset-2 z-30 flex items-center justify-center rounded-pw border-2 border-dashed border-accent bg-surface/80"
          data-testid="drop-overlay"
        >
          <p className="text-ui flex items-center gap-2 text-fg">
            <FileDown aria-hidden="true" className="size-5 text-accent" />
            {t('import.dropHint')}
          </p>
        </div>
      ) : null}
      {busy ? (
        <p role="status" className="sr-only">
          {t('import.running')}
        </p>
      ) : null}
      {question ? (
        <Dialog
          title={t('import.duplicateTitle')}
          onClose={() => {
            question.answer('skip');
          }}
          testId="duplicate-dialog"
          width="w-[min(480px,92vw)]"
        >
          <p className="text-ui">
            {t('import.duplicateText', { file: question.file, title: question.existing.title })}
          </p>
          <div className="mt-4 flex flex-wrap justify-end gap-2">
            <button
              type="button"
              data-autofocus
              data-testid="duplicate-existing"
              className="text-ui inline-flex h-7 items-center rounded-pw bg-accent px-3 font-medium text-surface"
              onClick={() => {
                question.answer('existing');
              }}
            >
              {t('import.useExisting')}
            </button>
            <button
              type="button"
              data-testid="duplicate-import"
              className="text-ui inline-flex h-7 items-center rounded-pw border border-muted px-3 text-fg"
              onClick={() => {
                question.answer('import');
              }}
            >
              {t('import.importAnyway')}
            </button>
            <button
              type="button"
              className="text-ui inline-flex h-7 items-center rounded-pw border border-muted px-3 text-fg"
              onClick={() => {
                question.answer('skip');
              }}
            >
              {t('import.skip')}
            </button>
          </div>
        </Dialog>
      ) : null}
    </>
  );
}
