/**
 * "Versionen" (stage 4, section 13; ADR 0016): the versions of a document,
 * two of them – or one and the current state – compared side by side, and
 * restoring a version. Restoring saves the current state as a version first
 * (Rust), so nothing is lost.
 */
import { useEffect, useMemo, useState } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useLocale, useT, type MessageKey } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { serializeMarkdown } from '@/modules/editor/markdown/adapter';
import { getSession } from '@/modules/editor/session';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { VersionKind } from '@/types/generated/VersionKind';
import type { VersionSummary } from '@/types/generated/VersionSummary';
import { changeCounts, diffLines, sideBySide, splitLines } from './diff';

interface HistoryStore {
  /** The document whose versions are shown. */
  documentId: string | null;
  /** The document a name is asked for ("Version speichern"). */
  naming: string | null;
  open: (id: string) => void;
  close: () => void;
  askName: (id: string | null) => void;
}

export const useHistory = create<HistoryStore>((set) => ({
  documentId: null,
  naming: null,
  open: (id) => {
    set({ documentId: id });
  },
  close: () => {
    set({ documentId: null });
  },
  askName: (id) => {
    set({ naming: id });
  },
}));

const CURRENT = 'current';
/** Rows shown at most; longer comparisons say how many were left out. */
const MAX_ROWS = 5000;

const KIND_KEYS: Record<VersionKind, MessageKey> = {
  auto: 'history.kind.auto',
  close: 'history.kind.close',
  beforeConversion: 'history.kind.beforeConversion',
  beforeRestore: 'history.kind.beforeRestore',
  named: 'history.kind.named',
};

/** The text that is compared: `.pwrite` as Markdown of its content. */
export function comparisonText(format: DocumentFormat, text: string): string {
  if (format !== 'pwrite') {
    return text;
  }
  try {
    return serializeMarkdown({ frontMatter: '', body: JSON.parse(text) as never });
  } catch {
    return text;
  }
}

export function HistoryDialog() {
  const documentId = useHistory((state) => state.documentId);
  return documentId === null ? null : <History documentId={documentId} />;
}

function History({ documentId }: { documentId: string }) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const document = useWorkspace((state) => state.open[documentId] ?? null);
  const summary = useWorkspace((state) => state.documents.find((entry) => entry.id === documentId));
  const [versions, setVersions] = useState<VersionSummary[] | null>(null);
  const [left, setLeft] = useState<string | null>(null);
  const [right, setRight] = useState<string>(CURRENT);
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [error, setError] = useState<CommandError | null>(null);
  // The current state as the open tab has it (unsaved changes included),
  // taken when the dialog opens.
  const [live] = useState(() => {
    const text = getSession(documentId)?.text();
    const format = useWorkspace.getState().open[documentId]?.format;
    return text === undefined || format === undefined ? undefined : comparisonText(format, text);
  });
  const dateTime = useMemo(
    () => new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }),
    [locale],
  );
  const close = () => {
    useHistory.getState().close();
  };

  useEffect(() => {
    let current = true;
    void call('document_versions', { id: documentId }).then((result) => {
      if (!current) {
        return;
      }
      if (result.ok) {
        setVersions(result.value);
        setLeft(result.value[0]?.id ?? null);
      } else {
        setError(result.error);
      }
    });
    return () => {
      current = false;
    };
  }, [documentId]);

  // The texts of the two sides, loaded once each.
  useEffect(() => {
    const wanted = [left, right].filter(
      (id): id is string => id !== null && texts[id] === undefined && !(id === CURRENT && live !== undefined),
    );
    for (const id of wanted) {
      if (id === CURRENT) {
        if (live === undefined) {
          void call('document_read', { id: documentId }).then((result) => {
            if (result.ok) {
              setTexts((previous) => ({
                ...previous,
                [CURRENT]: comparisonText(result.value.document.format, result.value.text),
              }));
            }
          });
        }
        continue;
      }
      void call('document_version_text', { id }).then((result) => {
        if (result.ok) {
          setTexts((previous) => ({
            ...previous,
            [id]: comparisonText(result.value.version.format, result.value.text),
          }));
        } else {
          setError(result.error);
        }
      });
    }
  }, [left, right, texts, documentId, live]);

  const textOf = (id: string | null) =>
    id === null ? undefined : id === CURRENT && live !== undefined ? live : texts[id];
  const leftText = textOf(left);
  const rightText = textOf(right);
  const comparison = useMemo(() => {
    if (leftText === undefined || rightText === undefined) {
      return null;
    }
    const lines = diffLines(splitLines(leftText), splitLines(rightText));
    return { rows: sideBySide(lines), counts: changeCounts(lines) };
  }, [leftText, rightText]);

  const label = (version: VersionSummary) =>
    `${dateTime.format(new Date(version.createdAt))} · ${version.name ?? t(KIND_KEYS[version.kind])}`;
  const leftVersion = versions?.find((version) => version.id === left) ?? null;

  const restore = async () => {
    if (!leftVersion) {
      return;
    }
    if (await useWorkspace.getState().restoreVersion(documentId, leftVersion.id)) {
      close();
    }
  };

  const options = (withCurrent: boolean) => (
    <>
      {withCurrent ? <option value={CURRENT}>{t('history.current')}</option> : null}
      {versions?.map((version) => (
        <option key={version.id} value={version.id}>
          {label(version)}
        </option>
      ))}
    </>
  );

  return (
    <Dialog
      title={t('history.title', { title: summary?.title ?? document?.title ?? '' })}
      onClose={close}
      testId="history-dialog"
      width="w-[min(1100px,96vw)]"
    >
      {versions === null && !error ? (
        <p role="status" className="text-ui text-muted">
          {t('history.loading')}
        </p>
      ) : null}
      {versions?.length === 0 ? (
        <p className="text-ui text-muted" data-testid="history-empty">
          {t('history.empty')}
        </p>
      ) : null}
      {versions && versions.length > 0 ? (
        <>
          <div className="text-ui flex flex-wrap items-end gap-3">
            <label className="flex flex-col gap-1">
              {t('history.left')}
              <select
                className="text-ui h-7 rounded-pw border border-muted bg-surface px-1 text-fg"
                value={left ?? ''}
                data-testid="history-left"
                onChange={(event) => {
                  setLeft(event.target.value);
                }}
              >
                {options(false)}
              </select>
            </label>
            <label className="flex flex-col gap-1">
              {t('history.right')}
              <select
                className="text-ui h-7 rounded-pw border border-muted bg-surface px-1 text-fg"
                value={right}
                data-testid="history-right"
                onChange={(event) => {
                  setRight(event.target.value);
                }}
              >
                {options(true)}
              </select>
            </label>
            {comparison ? (
              <p className="text-muted" data-testid="history-counts">
                {t('history.counts', {
                  removed: comparison.counts.removed,
                  added: comparison.counts.added,
                })}
              </p>
            ) : null}
            <button
              type="button"
              className="text-ui ml-auto inline-flex h-7 items-center rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-50"
              disabled={!leftVersion || (document?.readOnly ?? false)}
              data-testid="history-restore"
              onClick={() => void restore()}
            >
              {t('history.restore')}
            </button>
          </div>
          <p className="text-ui mt-1 text-muted">{t('history.restoreHint')}</p>
          {comparison ? (
            <ComparisonTable rows={comparison.rows} t={t} />
          ) : (
            <p role="status" className="text-ui mt-3 text-muted">
              {t('history.loading')}
            </p>
          )}
        </>
      ) : null}
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </Dialog>
  );
}

const MARK = { removed: '−', added: '+', same: ' ' } as const;

function ComparisonTable({ rows, t }: { rows: ReturnType<typeof sideBySide>; t: ReturnType<typeof useT> }) {
  const shown = rows.slice(0, MAX_ROWS);
  return (
    <div className="mt-3 max-h-[60vh] overflow-auto rounded-pw border border-muted/40">
      <table className="text-ui w-full table-fixed border-collapse font-mono" data-testid="history-table">
        <thead className="sticky top-0 bg-sidebar">
          <tr>
            <th className="w-1/2 px-2 py-1 text-left font-sans font-semibold">{t('history.left')}</th>
            <th className="w-1/2 px-2 py-1 text-left font-sans font-semibold">{t('history.right')}</th>
          </tr>
        </thead>
        <tbody>
          {shown.map((row, index) => (
            <tr key={index} className="align-top">
              <Cell side={row.left} />
              <Cell side={row.right} />
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length > MAX_ROWS ? (
        <p className="text-ui p-2 text-muted">{t('history.truncated', { count: rows.length - MAX_ROWS })}</p>
      ) : null}
    </div>
  );
}

function Cell({
  side,
}: {
  side: { number: number; text: string; kind: 'same' | 'removed' | 'added' } | null;
}) {
  if (!side) {
    return <td className="border-l border-muted/20 bg-sidebar/60" />;
  }
  const tone =
    side.kind === 'removed'
      ? 'border-l-2 border-l-danger bg-danger/10'
      : side.kind === 'added'
        ? 'border-l-2 border-l-accent bg-accent/10'
        : 'border-l border-muted/20';
  return (
    <td className={`px-2 py-0.5 break-words whitespace-pre-wrap ${tone}`} data-line={side.kind}>
      <span aria-hidden="true" className="mr-2 inline-block w-8 text-right text-muted select-none">
        {side.number}
      </span>
      <span aria-hidden="true" className="mr-1 select-none">
        {MARK[side.kind]}
      </span>
      {side.kind !== 'same' ? <span className="sr-only">{side.kind === 'removed' ? '−' : '+'}</span> : null}
      {side.text}
    </td>
  );
}

/** "Version speichern": asks for a name and stores the current state. */
export function VersionNameDialog() {
  const documentId = useHistory((state) => state.naming);
  return documentId === null ? null : <NameForm documentId={documentId} />;
}

function NameForm({ documentId }: { documentId: string }) {
  const t = useT();
  const [name, setName] = useState('');
  const [error, setError] = useState<CommandError | null>(null);
  const close = () => {
    useHistory.getState().askName(null);
  };
  const save = async () => {
    const result = await useWorkspace.getState().saveVersion(documentId, name);
    if (result === null) {
      close();
    } else {
      setError(result);
    }
  };
  return (
    <Dialog
      title={t('history.saveTitle')}
      onClose={close}
      testId="version-name-dialog"
      width="w-[min(440px,92vw)]"
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <label className="text-ui flex flex-col gap-1">
          {t('history.name')}
          <input
            type="text"
            maxLength={200}
            className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
            value={name}
            autoFocus
            data-testid="version-name"
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
        </label>
        {error ? (
          <p role="alert" className="text-ui text-danger">
            {errorText(t, error)}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <button type="button" className="text-ui h-7 rounded-pw border border-muted px-3" onClick={close}>
            {t('history.cancel')}
          </button>
          <button
            type="submit"
            className="text-ui h-7 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-50"
            disabled={name.trim() === ''}
          >
            {t('history.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
