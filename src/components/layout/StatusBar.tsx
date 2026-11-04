/**
 * Status bar (section 11, 24 px). Library documents show "Gespeichert",
 * "Speichert …" or "Fehler beim Speichern" (section 9). External files
 * show "Nicht gespeichert" while they have changes, their folder, and
 * their encoding and line endings, which can be changed here (ADR 0015).
 */
import { CircleAlert, FileText, Keyboard, Library, Lock, TriangleAlert } from 'lucide-react';
import { useLocale, useT } from '@/i18n';
import type { DocumentView, SaveState } from '@/modules/documents/workspaceStore';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { LibraryInfo } from '@/types/generated/LibraryInfo';
import type { LineEnding } from '@/types/generated/LineEnding';
import type { TextEncoding } from '@/types/generated/TextEncoding';

/** The encodings a text file can be saved in. */
export const ENCODINGS: readonly TextEncoding[] = [
  'utf8',
  'utf8Bom',
  'utf16Le',
  'utf16Be',
  'utf16LeNoBom',
  'utf16BeNoBom',
  'windows1252',
];
const LINE_ENDINGS: readonly LineEnding[] = ['lf', 'crlf'];

/** What the status bar shows of an external file. */
export interface ExternalStatus {
  folder: string;
  encoding: TextEncoding;
  lineEnding: LineEnding;
  /** `.pwrite` is always UTF-8 with LF: nothing to choose. */
  textFile: boolean;
  fileReadOnly: boolean;
  unsaved: boolean;
  onEncoding: (encoding: TextEncoding) => void;
  onLineEnding: (lineEnding: LineEnding) => void;
}

interface StatusBarProps {
  library: LibraryInfo | null;
  saveState: SaveState | null;
  format: DocumentFormat | null;
  /** Rich text or source of a Markdown document. */
  view: DocumentView | null;
  zoom: number;
  conflicts: number;
  /** Words of the document and the selection; reading time if switched on. */
  statistics: { words: number; selectedWords: number | null; minutes: number | null } | null;
  /** Set while the active tab shows an external file. */
  external: ExternalStatus | null;
  onShowStatistics: () => void;
  onRetrySave: () => void;
  onResetZoom: () => void;
  onShowConflicts: () => void;
}

const ITEM = 'text-ui inline-flex h-full items-center gap-1.5 px-2';
const BUTTON = `${ITEM} rounded-pw hover:bg-surface`;
const SELECT =
  'text-ui h-5 rounded-pw border-0 bg-transparent px-1 text-muted hover:bg-surface hover:text-fg';

export function StatusBar({
  library,
  saveState,
  format,
  view,
  zoom,
  conflicts,
  statistics,
  external,
  onShowStatistics,
  onRetrySave,
  onResetZoom,
  onShowConflicts,
}: StatusBarProps) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const number = new Intl.NumberFormat(locale);
  const formatLabel =
    format === 'md'
      ? t(view === 'source' ? 'status.formatMarkdown' : 'status.formatMarkdownRich')
      : format === 'txt'
        ? t('status.formatText')
        : format === 'pwrite'
          ? t('status.formatPwrite')
          : null;
  return (
    <footer
      aria-label={t('status.label')}
      data-testid="status-bar"
      className="flex h-6 shrink-0 items-center justify-between gap-2 border-t border-muted/40 bg-sidebar text-muted"
    >
      <div className="flex h-full min-w-0 items-center">
        {library ? (
          <span className={ITEM} title={library.displayPath} data-testid="status-library">
            <Library aria-hidden="true" className="size-3.5" />
            <span className="truncate">{library.name}</span>
          </span>
        ) : null}
        {library?.journalMode === 'rollback' ? (
          <span className={ITEM} title={t('status.rollbackHint')} data-testid="status-rollback">
            <TriangleAlert aria-hidden="true" className="size-3.5 text-danger" />
            {t('status.rollback')}
          </span>
        ) : null}
        <span aria-live="polite" className="contents">
          {saveState === 'error' ? (
            <button
              type="button"
              className={`${BUTTON} text-danger`}
              data-testid="status-save"
              onClick={onRetrySave}
            >
              <CircleAlert aria-hidden="true" className="size-3.5" />
              {t('status.saveError')} – {t('status.retry')}
            </button>
          ) : saveState ? (
            <span className={ITEM} data-testid="status-save" data-state={saveState}>
              {saveState === 'blocked'
                ? t('status.blocked')
                : saveState === 'saving'
                  ? t('status.saving')
                  : external?.unsaved
                    ? t('status.unsaved')
                    : saveState === 'saved'
                      ? t('status.saved')
                      : t('status.saving')}
            </span>
          ) : null}
        </span>
        {external ? (
          <span className={ITEM} title={external.folder} data-testid="status-external">
            <FileText aria-hidden="true" className="size-3.5" />
            <span className="max-w-64 truncate">{t('status.externalFile', { folder: external.folder })}</span>
          </span>
        ) : null}
        {external?.fileReadOnly ? (
          <span className={ITEM} data-testid="status-file-read-only">
            <Lock aria-hidden="true" className="size-3.5" />
            {t('status.fileReadOnly')}
          </span>
        ) : null}
      </div>
      <div className="flex h-full items-center">
        {conflicts > 0 ? (
          <button
            type="button"
            className={`${BUTTON} text-danger`}
            data-testid="status-conflicts"
            onClick={onShowConflicts}
          >
            <Keyboard aria-hidden="true" className="size-3.5" />
            {t('status.conflicts', { count: conflicts })}
          </button>
        ) : null}
        {statistics ? (
          <button
            type="button"
            className={BUTTON}
            data-testid="status-words"
            title={t('statistics.title')}
            onClick={onShowStatistics}
          >
            {statistics.selectedWords !== null
              ? t('status.wordsSelected', {
                  selected: number.format(statistics.selectedWords),
                  count: number.format(statistics.words),
                })
              : t('status.words', { count: number.format(statistics.words) })}
            {statistics.minutes !== null
              ? ` · ${t('statistics.minutes', { count: number.format(statistics.minutes) })}`
              : ''}
          </button>
        ) : null}
        {external?.textFile ? (
          <>
            <select
              className={SELECT}
              aria-label={t('status.encoding')}
              title={t('status.encoding')}
              data-testid="status-encoding"
              value={external.encoding}
              onChange={(event) => {
                external.onEncoding(event.target.value as TextEncoding);
              }}
            >
              {ENCODINGS.map((encoding) => (
                <option key={encoding} value={encoding}>
                  {t(`encoding.${encoding}`)}
                </option>
              ))}
            </select>
            <select
              className={SELECT}
              aria-label={t('status.lineEnding')}
              title={t('status.lineEnding')}
              data-testid="status-line-ending"
              value={external.lineEnding}
              onChange={(event) => {
                external.onLineEnding(event.target.value as LineEnding);
              }}
            >
              {LINE_ENDINGS.map((lineEnding) => (
                <option key={lineEnding} value={lineEnding}>
                  {t(`lineEnding.${lineEnding}`)}
                </option>
              ))}
            </select>
          </>
        ) : null}
        {formatLabel ? <span className={ITEM}>{formatLabel}</span> : null}
        {Math.abs(zoom - 1) > 1e-9 ? (
          <button
            type="button"
            className={BUTTON}
            data-testid="status-zoom"
            title={t('command.zoom.reset')}
            onClick={onResetZoom}
          >
            {t('status.zoom', { percent: Math.round(zoom * 100) })}
          </button>
        ) : null}
      </div>
    </footer>
  );
}
