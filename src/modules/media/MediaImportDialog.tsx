/**
 * The import dialog (section 9, "Import in die Bibliothek"; ADR 0019,
 * decisions 5 and 6): while Rust copies and checks the files it shows the
 * progress and can cancel; then it lists what will be taken over, asks
 * about duplicates ("vorhandenes Element verwenden oder trotzdem
 * importieren"), names what is skipped and why, and offers subnets from
 * the folder structure. Cancelling at any point leaves nothing behind.
 */
import { useId, useMemo, useState } from 'react';
import { Dialog } from '@/components/Dialog';
import { useLocale, useT, type MessageKey } from '@/i18n';
import { formatBytes } from '@/modules/library/format';
import { useStructure } from '@/modules/structure/structureStore';
import { buildIndex, pathLabel } from '@/modules/structure/tree';
import type { Decision } from '@/types/generated/Decision';
import type { ImportPreview } from '@/types/generated/ImportPreview';
import type { SkipReason } from '@/types/generated/SkipReason';
import { useMedia, type ImportSource } from './mediaStore';

/** Rows of a long list shown at once (the rest is counted). */
const SHOWN_ITEMS = 300;

const TITLES: Record<ImportSource, MessageKey> = {
  files: 'media.import.title',
  folder: 'media.import.titleFolder',
  drop: 'media.import.titleDrop',
};

export function MediaImportDialog() {
  const state = useMedia((store) => store.importing);
  if (!state) {
    return null;
  }
  return state.phase === 'preparing' ? (
    <Preparing source={state.source} done={state.done} total={state.total} />
  ) : (
    <Review
      key={state.preview.batchId}
      preview={state.preview}
      source={state.source}
      committing={state.committing}
    />
  );
}

function Preparing({ source, done, total }: { source: ImportSource; done: number; total: number }) {
  const t = useT();
  const cancel = () => void useMedia.getState().cancelImport();
  return (
    <Dialog title={t(TITLES[source])} onClose={cancel} testId="media-import-progress">
      <p className="text-ui" role="status" aria-live="polite">
        {total > 0 ? t('media.import.preparing', { done, total }) : t('media.import.preparingStart')}
      </p>
      {total > 0 ? (
        <progress
          className="mt-3 w-full"
          max={total}
          value={done}
          aria-label={t('media.import.preparingStart')}
        />
      ) : null}
      <div className="mt-4 flex justify-end">
        <button
          type="button"
          className="text-ui h-7 rounded-pw border border-muted/60 px-3 hover:bg-surface"
          data-testid="media-import-cancel"
          onClick={cancel}
        >
          {t('media.cancel')}
        </button>
      </div>
    </Dialog>
  );
}

function Review({
  preview,
  source,
  committing,
}: {
  preview: ImportPreview;
  source: ImportSource;
  committing: boolean;
}) {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const subnets = useStructure((store) => store.subnets);
  const index = useMemo(() => buildIndex(subnets, locale), [subnets, locale]);
  const [duplicates, setDuplicates] = useState<Decision>('useExisting');
  const [decisions, setDecisions] = useState<Record<number, Decision>>({});
  const [fromFolders, setFromFolders] = useState(source === 'folder');
  const [subnetId, setSubnetId] = useState<string>('');
  const subnetFieldId = useId();
  const duplicateItems = preview.items.filter((item) => item.duplicate !== null);
  const decisionOf = (index: number, duplicate: boolean): Decision =>
    decisions[index] ?? (duplicate ? duplicates : 'import');
  const importing = preview.items.filter(
    (item) => decisionOf(item.index, item.duplicate !== null) !== 'skip',
  );
  const nameOf = (index: number) => preview.items.find((item) => item.index === index)?.name ?? '';
  const size = (bytes: number) => {
    const formatted = formatBytes(bytes, locale);
    return t(formatted.key, { value: formatted.value });
  };
  const sortedSubnets = useMemo(
    () =>
      subnets
        .map((subnet) => ({ id: subnet.id, label: pathLabel(index, subnet.id) }))
        .sort((a, b) => a.label.localeCompare(b.label, locale)),
    [subnets, index, locale],
  );
  const cancel = () => void useMedia.getState().cancelImport();
  const commit = () =>
    void useMedia.getState().commitImport({
      decisions,
      duplicates,
      subnetId: subnetId === '' ? null : subnetId,
      fromFolders: source === 'folder' && fromFolders,
    });

  return (
    <Dialog title={t(TITLES[source])} onClose={cancel} testId="media-import" width="min(44rem, 92vw)">
      <div className="text-ui flex max-h-[70vh] flex-col gap-3 overflow-auto">
        {preview.folderName ? (
          <p className="text-muted">{t('media.import.folder', { name: preview.folderName })}</p>
        ) : null}
        <p className="text-muted">{t('media.import.originals')}</p>
        {preview.items.length === 0 ? (
          <p role="alert" data-testid="media-import-nothing">
            {t('media.import.nothing')}
          </p>
        ) : (
          <section aria-label={t('media.import.items', { count: importing.length })}>
            <h3 className="font-semibold" data-testid="media-import-count">
              {t('media.import.items', { count: importing.length })}
            </h3>
            {duplicateItems.length > 0 ? (
              <label className="mt-2 flex items-center gap-2">
                <span>{t('media.import.duplicatesAll')}</span>
                <select
                  className="h-7 rounded-pw border border-muted/60 bg-surface px-1 text-fg"
                  value={duplicates}
                  data-testid="media-import-duplicates"
                  onChange={(event) => {
                    setDuplicates(event.target.value as Decision);
                    setDecisions({});
                  }}
                >
                  <option value="useExisting">{t('media.import.useExisting')}</option>
                  <option value="import">{t('media.import.importAnyway')}</option>
                  <option value="skip">{t('media.import.skip')}</option>
                </select>
              </label>
            ) : null}
            <ul className="mt-2 flex flex-col gap-1" data-testid="media-import-items">
              {preview.items.slice(0, SHOWN_ITEMS).map((item) => (
                <li
                  key={item.index}
                  className="flex flex-wrap items-center gap-2"
                  data-import-item={item.name}
                >
                  <span className="min-w-0 flex-1 truncate">
                    {item.folder ? <span className="text-muted">{item.folder}/</span> : null}
                    {item.name}
                  </span>
                  <span className="text-muted">
                    {t(`media.kind.${item.kind}`)} · {size(item.sizeBytes)}
                  </span>
                  {item.duplicate ? (
                    <span className="flex w-full items-center gap-2 pl-4 text-muted" data-duplicate>
                      {item.duplicate.kind === 'existing'
                        ? t('media.import.duplicateExisting', { title: item.duplicate.title })
                        : t('media.import.duplicateInBatch', { name: nameOf(item.duplicate.index) })}
                      <select
                        className="h-6 rounded-pw border border-muted/60 bg-surface px-1 text-fg"
                        value={decisionOf(item.index, true)}
                        aria-label={item.name}
                        onChange={(event) => {
                          const value = event.target.value as Decision;
                          setDecisions((previous) => ({ ...previous, [item.index]: value }));
                        }}
                      >
                        <option value="useExisting">{t('media.import.useExisting')}</option>
                        <option value="import">{t('media.import.importAnyway')}</option>
                        <option value="skip">{t('media.import.skip')}</option>
                      </select>
                    </span>
                  ) : null}
                </li>
              ))}
            </ul>
            {preview.items.length > SHOWN_ITEMS ? (
              <p className="mt-1 text-muted">+ {preview.items.length - SHOWN_ITEMS}</p>
            ) : null}
          </section>
        )}
        {preview.skipped.length > 0 ? (
          <details data-testid="media-import-skipped" open={preview.items.length === 0}>
            <summary className="cursor-default font-semibold">
              {t('media.import.skipped', { count: preview.skipped.length })}
            </summary>
            <ul className="mt-1 flex flex-col gap-1">
              {preview.skipped.slice(0, SHOWN_ITEMS).map((item, position) => (
                <li
                  key={`${item.folder}/${item.name}/${String(position)}`}
                  className="flex gap-2"
                  data-skipped={item.name}
                >
                  <span className="min-w-0 flex-1 truncate">
                    {item.folder ? <span className="text-muted">{item.folder}/</span> : null}
                    {item.name}
                  </span>
                  <span className="text-muted" data-reason={item.reason}>
                    {t(`media.skip.${item.reason satisfies SkipReason}`)}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        ) : null}
        {preview.items.length > 0 ? (
          <fieldset className="flex flex-col gap-2">
            {source === 'folder' ? (
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={fromFolders}
                  data-testid="media-import-from-folders"
                  onChange={(event) => {
                    setFromFolders(event.target.checked);
                  }}
                />
                {t('media.import.fromFolders')}
              </label>
            ) : null}
            <label htmlFor={subnetFieldId} className="flex items-center gap-2">
              <span>
                {source === 'folder' && fromFolders
                  ? t('media.import.subnetBelow')
                  : t('media.import.subnet')}
              </span>
              <select
                id={subnetFieldId}
                className="h-7 min-w-0 flex-1 rounded-pw border border-muted/60 bg-surface px-1 text-fg"
                value={subnetId}
                data-testid="media-import-subnet"
                onChange={(event) => {
                  setSubnetId(event.target.value);
                }}
              >
                <option value="">
                  {source === 'folder' && fromFolders
                    ? t('media.import.topLevel')
                    : t('media.import.noSubnet')}
                </option>
                {sortedSubnets.map((subnet) => (
                  <option key={subnet.id} value={subnet.id}>
                    {subnet.label}
                  </option>
                ))}
              </select>
            </label>
          </fieldset>
        ) : null}
      </div>
      <div className="mt-4 flex justify-end gap-2">
        <button
          type="button"
          className="text-ui h-7 rounded-pw border border-muted/60 px-3 hover:bg-surface"
          data-testid="media-import-cancel"
          onClick={cancel}
        >
          {t('media.cancel')}
        </button>
        {preview.items.length > 0 ? (
          <button
            type="button"
            className="text-ui h-7 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-60"
            disabled={committing || importing.length === 0}
            data-testid="media-import-confirm"
            onClick={commit}
          >
            {t('media.import.confirm')}
          </button>
        ) : null}
      </div>
    </Dialog>
  );
}
