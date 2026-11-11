/**
 * Opens a hit of the global search "an der Fundstelle" (section 19).
 *
 * * A document opens in its tab; the find bar searches the first marked
 *   word of the snippet and the first result is selected and scrolled to.
 * * A medium opens in its view.
 * * A subnet, and anything in the trash, is shown in the Explorer.
 */
import { useLocale } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useExplorer } from '@/modules/documents/explorerStore';
import { useFind } from '@/modules/editor/find/findStore';
import { getSession } from '@/modules/editor/session';
import { useUi } from '@/modules/settings/uiStore';
import { useStructure } from '@/modules/structure/structureStore';
import { buildIndex, pathTo } from '@/modules/structure/tree';
import type { SearchHit } from '@/types/generated/SearchHit';

/** How long to wait for the editor of a document that is opening. */
const EDITOR_WAIT_MS = 3000;

/** The first marked word of a snippet, if any. */
export function firstHitText(hit: SearchHit): string | null {
  const part = hit.snippet.find((piece) => piece.hit && piece.text.trim() !== '');
  return part ? part.text.trim() : null;
}

function showInExplorer(key: string, path: readonly string[] = []): void {
  useUi.getState().setSideView('explorer');
  useExplorer.getState().revealRow(key, path);
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 16);
  });
}

/** Waits for the editor of `id`, then finds `term` in it. */
async function findInDocument(id: string, term: string): Promise<void> {
  const until = performance.now() + EDITOR_WAIT_MS;
  while (performance.now() < until) {
    const document = useWorkspace.getState().open[id];
    const session = getSession(id);
    if (document?.loaded && session) {
      useFind.getState().open(id, false, term);
      // The find bar runs the search when it renders.
      for (let frame = 0; frame < 30; frame += 1) {
        await nextFrame();
        if (session.searchResults().count > 0) {
          session.revealFirstResult();
          return;
        }
      }
      return;
    }
    await nextFrame();
  }
}

export async function openHit(hit: SearchHit): Promise<void> {
  const media = hit.kind === 'image' || hit.kind === 'pdf' || hit.kind === 'attachment';
  if (hit.kind === 'subnet') {
    const { subnets } = useStructure.getState();
    const index = buildIndex(subnets, useLocale.getState().locale);
    const path = pathTo(index, hit.id).map((subnet) => subnet.id);
    showInExplorer(`s:${hit.id}`, path.slice(0, -1));
    return;
  }
  if (hit.deleted) {
    useExplorer.getState().toggleGroup('trash', true);
    showInExplorer(media ? `trash:m-${hit.id}` : `trash:${hit.id}`);
    return;
  }
  if (media) {
    useWorkspace.getState().openMedia(hit.id);
    return;
  }
  const opened = await useWorkspace.getState().openDocument(hit.id);
  const term = firstHitText(hit);
  if (opened && term) {
    await findInDocument(hit.id, term);
  }
}
