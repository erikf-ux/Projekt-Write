/**
 * One medium in a tab (section 17): an image (fitted or in its own size),
 * a PDF (PdfView) or an attachment (metadata; opened with the standard
 * program as a read-only copy), with its details, the documents that use
 * it and the actions of the Explorer.
 */
import {
  Archive,
  ArchiveRestore,
  ExternalLink,
  FolderInput,
  Maximize,
  Scan,
  Star,
  Tags,
  Trash2,
  Undo2,
} from 'lucide-react';
import { lazy, Suspense, useEffect, useId, useState } from 'react';
import { useLocale, useT } from '@/i18n';
import { formatBytes } from '@/modules/library/format';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useStructureDialogs } from '@/modules/structure/dialogStore';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import { mediaById, useMedia } from './mediaStore';
import { mediaUrl } from './mediaUrl';

// PDF.js loads with the first PDF, not with the app (cold start, section 6).
const PdfView = lazy(() => import('./PdfView').then((module) => ({ default: module.PdfView })));

interface MediaViewProps {
  mediaId: string;
  page: number | null;
  hidden: boolean;
}

export function MediaView({ mediaId, page, hidden }: MediaViewProps) {
  const t = useT();
  const medium = useMedia((state) => mediaById(state, mediaId));
  const loaded = useMedia((state) => state.loaded);

  useEffect(() => {
    if (!hidden) {
      void useMedia.getState().markOpened(mediaId);
      void useMedia.getState().loadUses(mediaId);
    }
  }, [mediaId, hidden]);

  if (!medium) {
    return loaded ? (
      <p className="text-ui p-6 text-muted" role="alert" data-testid="media-missing">
        {t('media.view.missing')}
      </p>
    ) : null;
  }
  return (
    <div
      className="flex min-h-0 flex-1"
      data-testid="media-view"
      data-media={medium.id}
      data-kind={medium.kind}
    >
      <div className="flex min-w-0 flex-1 flex-col">
        <MediaHeader medium={medium} />
        {medium.kind === 'image' ? <ImageBody medium={medium} /> : null}
        {medium.kind === 'pdf' ? (
          <Suspense fallback={<p className="text-ui p-6 text-muted">{t('pdf.loading')}</p>}>
            <PdfView mediaId={medium.id} page={page} needsThumbnail={!medium.pageCount} />
          </Suspense>
        ) : null}
        {medium.kind === 'attachment' ? <AttachmentBody medium={medium} /> : null}
      </div>
      <MediaDetails medium={medium} />
    </div>
  );
}

function ActionButton({
  label,
  icon: Icon,
  onClick,
  pressed,
  testId,
}: {
  label: string;
  icon: typeof Star;
  onClick: () => void;
  pressed?: boolean;
  testId?: string;
}) {
  return (
    <button
      type="button"
      className={`inline-flex size-7 items-center justify-center rounded-pw hover:bg-surface ${
        pressed ? 'text-accent' : 'text-fg'
      }`}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function MediaHeader({ medium }: { medium: MediaSummary }) {
  const t = useT();
  const [title, setTitle] = useState(medium.title);
  const [shown, setShown] = useState(medium.title);
  if (medium.title !== shown) {
    setShown(medium.title);
    setTitle(medium.title);
  }
  const store = useMedia.getState;
  const inTrash = medium.deletedAt !== null;
  const save = () => {
    const next = title.trim();
    if (next === '' || next === medium.title) {
      setTitle(medium.title);
      return;
    }
    void store().update(medium.id, { title: next });
  };
  return (
    <header className="flex h-10 shrink-0 items-center gap-2 border-b border-muted/40 px-3">
      <input
        aria-label={t('media.view.title')}
        className="text-ui h-7 min-w-0 flex-1 rounded-pw border border-transparent bg-transparent px-1 font-semibold text-fg hover:border-muted/60 focus:border-accent"
        value={title}
        maxLength={300}
        disabled={inTrash}
        data-testid="media-title"
        onChange={(event) => {
          setTitle(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.currentTarget.blur();
          } else if (event.key === 'Escape') {
            setTitle(medium.title);
          }
        }}
      />
      {inTrash ? <span className="text-ui text-muted">{t('media.view.inTrash')}</span> : null}
      {medium.archivedAt !== null && !inTrash ? (
        <span className="text-ui text-muted">{t('media.view.archived')}</span>
      ) : null}
      <div role="toolbar" aria-label={t('media.view.actions')} className="flex items-center">
        {inTrash ? (
          <>
            <ActionButton
              label={t('media.view.restore')}
              icon={Undo2}
              onClick={() => void store().restore(medium.id)}
            />
            <ActionButton
              label={t('media.view.purge')}
              icon={Trash2}
              onClick={() => void store().purge(medium.id)}
            />
          </>
        ) : (
          <>
            <ActionButton
              label={t('media.view.openExternal')}
              icon={ExternalLink}
              testId="media-open-external"
              onClick={() => void store().openExternal(medium.id)}
            />
            <ActionButton
              label={medium.favoritedAt ? t('media.view.unfavorite') : t('media.view.favorite')}
              icon={Star}
              pressed={medium.favoritedAt !== null}
              onClick={() => void store().setFavorite(medium.id, medium.favoritedAt === null)}
            />
            <ActionButton
              label={t('structure.moveTo')}
              icon={FolderInput}
              onClick={() => {
                useStructureDialogs.getState().openPicker({ mode: 'moveDocument', documentId: medium.id });
              }}
            />
            <ActionButton
              label={t('tags.edit')}
              icon={Tags}
              onClick={() => {
                useStructureDialogs.getState().openTags(medium.id);
              }}
            />
            <ActionButton
              label={medium.archivedAt ? t('media.view.unarchive') : t('media.view.archive')}
              icon={medium.archivedAt ? ArchiveRestore : Archive}
              onClick={() => void store().archive(medium.id, medium.archivedAt === null)}
            />
            <ActionButton
              label={t('media.view.delete')}
              icon={Trash2}
              testId="media-delete"
              onClick={() => void store().remove(medium.id)}
            />
          </>
        )}
      </div>
    </header>
  );
}

function ImageBody({ medium }: { medium: MediaSummary }) {
  const t = useT();
  const [fit, setFit] = useState(true);
  const [failed, setFailed] = useState(false);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-1 border-b border-muted/40 bg-sidebar px-2">
        <ActionButton
          label={t('media.view.zoomFit')}
          icon={Scan}
          pressed={fit}
          onClick={() => {
            setFit(true);
          }}
        />
        <ActionButton
          label={t('media.view.zoomActual')}
          icon={Maximize}
          pressed={!fit}
          onClick={() => {
            setFit(false);
          }}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-auto bg-sidebar p-4" tabIndex={0} aria-label={medium.title}>
        {failed ? (
          <p className="text-ui text-muted">{t('editor.mediaMissing')}</p>
        ) : (
          // An image of the library, through the media handler: an SVG is
          // only ever an image source, never part of the DOM (section 17).
          <img
            src={mediaUrl(medium.id)}
            alt={medium.description || t('media.view.alt', { title: medium.title })}
            data-testid="media-image"
            className={fit ? 'mx-auto max-h-full max-w-full object-contain' : 'max-w-none'}
            onError={() => {
              setFailed(true);
            }}
          />
        )}
      </div>
    </div>
  );
}

function AttachmentBody({ medium }: { medium: MediaSummary }) {
  const t = useT();
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4 p-6 text-center">
      <p className="text-ui max-w-md text-muted">{t('media.attachment.text')}</p>
      <button
        type="button"
        className="text-ui inline-flex h-8 items-center gap-2 rounded-pw bg-accent px-3 font-medium text-surface"
        data-testid="attachment-open"
        onClick={() => void useMedia.getState().openExternal(medium.id)}
      >
        <ExternalLink aria-hidden="true" className="size-4" />
        {t('media.view.openExternal')}
      </button>
    </div>
  );
}

function MediaDetails({ medium }: { medium: MediaSummary }) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const uses = useMedia((state) => state.uses[medium.id]);
  const documents = useWorkspace((state) => state.documents);
  const [description, setDescription] = useState(medium.description);
  const [shown, setShown] = useState(medium.description);
  if (medium.description !== shown) {
    setShown(medium.description);
    setDescription(medium.description);
  }
  const descriptionId = useId();
  const size = formatBytes(medium.sizeBytes, locale);
  const date = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' });
  const rows: [string, string][] = [
    [t('media.view.type'), `${t(`media.kind.${medium.kind}`)} (${medium.mimeType})`],
    [t('media.view.size'), t(size.key, { value: size.value })],
  ];
  if (medium.width && medium.height) {
    rows.push([
      t('media.view.dimensions'),
      t('media.view.dimensionsValue', { width: medium.width, height: medium.height }),
    ]);
  }
  if (medium.pageCount) {
    rows.push([t('media.view.pages'), String(medium.pageCount)]);
  }
  rows.push([t('media.view.originalName'), medium.originalName]);
  if (medium.importFolder) {
    rows.push([t('media.view.folder'), medium.importFolder]);
  }
  rows.push([t('media.view.created'), date.format(new Date(medium.createdAt))]);
  const titleOf = (id: string) => documents.find((document) => document.id === id)?.title ?? id;
  return (
    <aside
      className="text-ui flex w-72 shrink-0 flex-col gap-3 overflow-auto border-l border-muted/40 bg-sidebar p-3"
      aria-label={t('media.view.info')}
      data-testid="media-details"
    >
      <h3 className="font-semibold text-muted uppercase">{t('media.view.info')}</h3>
      <dl className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1">
        {rows.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-muted">{label}</dt>
            <dd className="break-words">{value}</dd>
          </div>
        ))}
      </dl>
      <label htmlFor={descriptionId} className="font-semibold text-muted">
        {t('media.view.description')}
      </label>
      <textarea
        id={descriptionId}
        className="min-h-20 rounded-pw border border-muted/60 bg-surface p-1 text-fg"
        maxLength={2000}
        value={description}
        disabled={medium.deletedAt !== null}
        onChange={(event) => {
          setDescription(event.target.value);
        }}
        onBlur={() => {
          if (description.trim() !== medium.description) {
            void useMedia.getState().update(medium.id, { description });
          }
        }}
      />
      <h3 className="font-semibold text-muted uppercase">{t('media.view.uses')}</h3>
      {uses && uses.length > 0 ? (
        <ul className="flex flex-col gap-1" data-testid="media-uses">
          {uses.map((use) => (
            <li key={`${use.documentId}:${String(use.page)}`}>
              <button
                type="button"
                className="text-left text-accent hover:underline"
                onClick={() => void useWorkspace.getState().openDocument(use.documentId)}
              >
                {titleOf(use.documentId)}
                {use.page > 0 ? ` (${t('media.view.page', { page: use.page })})` : ''}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted">{t('media.view.usesNone')}</p>
      )}
    </aside>
  );
}
