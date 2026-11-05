/**
 * Breadcrumbs above the writing surface (Phase 7, "Navigation … über
 * Breadcrumbs"; ADR 0017, decision 12): the path of the primary subnet of
 * the active document, its further subnets, and its tags. A step shows its
 * subnet in the Explorer; the tags open the tags dialog.
 */
import { Tags } from 'lucide-react';
import { useMemo } from 'react';
import { useLocale, useT } from '@/i18n';
import { ColorDot } from '@/modules/documents/Explorer';
import { useExplorer } from '@/modules/documents/explorerStore';
import { useStructureDialogs } from './dialogStore';
import { NO_PLACEMENT, useStructure } from './structureStore';
import { buildIndex, pathLabel, pathTo } from './tree';

interface BreadcrumbsProps {
  documentId: string;
  title: string;
}

export function Breadcrumbs({ documentId, title }: BreadcrumbsProps) {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const subnets = useStructure((store) => store.subnets);
  const tags = useStructure((store) => store.tags);
  const placement = useStructure((store) => store.byDocument[documentId] ?? NO_PLACEMENT);
  const index = useMemo(() => buildIndex(subnets, locale), [subnets, locale]);
  const known = placement.subnetIds.filter((id) => index.byId.has(id));
  const primary = known[0];
  const path = primary ? pathTo(index, primary) : [];
  const others = known.slice(1);
  const own = placement.tagIds
    .map((id) => tags.find((tag) => tag.id === id))
    .filter((tag): tag is NonNullable<typeof tag> => tag !== undefined);

  const show = (subnetId: string) => {
    const ids = pathTo(index, subnetId).map((subnet) => subnet.id);
    useExplorer.getState().revealRow(`s:${subnetId}`, ids.slice(0, -1));
  };

  return (
    <nav
      aria-label={t('breadcrumbs.label')}
      data-testid="breadcrumbs"
      className="text-ui flex h-7 shrink-0 items-center gap-2 overflow-hidden border-b border-muted/30 px-3 text-muted"
    >
      <ol className="flex min-w-0 items-center gap-1">
        {path.length === 0 ? (
          <li>
            <button
              type="button"
              data-crumb="unassigned"
              className="rounded-pw px-1 hover:bg-sidebar hover:text-fg"
              onClick={() => {
                useExplorer.getState().revealRow(`unassigned:${documentId}`);
              }}
            >
              {t('explorer.group.unassigned')}
            </button>
          </li>
        ) : null}
        {path.map((subnet) => (
          <li key={subnet.id} className="flex min-w-0 items-center gap-1">
            <button
              type="button"
              data-crumb={subnet.id}
              className="inline-flex min-w-0 items-center gap-1 rounded-pw px-1 hover:bg-sidebar hover:text-fg"
              onClick={() => {
                show(subnet.id);
              }}
            >
              <ColorDot color={subnet.color} />
              <span className="truncate">{subnet.name}</span>
            </button>
            <span aria-hidden="true">›</span>
          </li>
        ))}
        <li className="min-w-0 truncate text-fg" aria-current="page">
          {title}
        </li>
      </ol>
      {others.length > 0 ? (
        <span className="flex min-w-0 shrink items-center gap-1 truncate" data-testid="breadcrumbs-others">
          {t('breadcrumbs.alsoIn')}
          {others.map((id, position) => (
            <button
              key={id}
              type="button"
              data-crumb-other={id}
              title={pathLabel(index, id)}
              className="rounded-pw px-1 hover:bg-sidebar hover:text-fg"
              onClick={() => {
                show(id);
              }}
            >
              {index.byId.get(id)?.name}
              {position < others.length - 1 ? ',' : ''}
            </button>
          ))}
        </span>
      ) : null}
      <button
        type="button"
        data-testid="breadcrumbs-tags"
        className="ml-auto inline-flex shrink-0 items-center gap-1 rounded-pw px-1 hover:bg-sidebar hover:text-fg"
        aria-label={
          own.length > 0
            ? t('breadcrumbs.tags', { tags: own.map((tag) => tag.name).join(', ') })
            : t('tags.edit')
        }
        onClick={() => {
          useStructureDialogs.getState().openTags(documentId);
        }}
      >
        <Tags aria-hidden="true" className="size-3.5" />
        {own.length > 0 ? own.map((tag) => `#${tag.name}`).join(' ') : t('breadcrumbs.noTags')}
      </button>
    </nav>
  );
}
