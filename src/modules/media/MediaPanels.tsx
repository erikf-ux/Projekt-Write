/**
 * The panels of the view tabs: the gallery and one per open medium. Like
 * document panels, all stay mounted and only the active one is shown, so a
 * PDF keeps its page and zoom while another tab is in front.
 */
import { panelElementId } from '@/modules/editor/DocumentEditor';
import type { ViewTab } from '@/modules/documents/workspaceStore';
import { MeshPanel } from '@/modules/mesh/MeshPanel';
import { Gallery } from './Gallery';
import { MediaView } from './MediaView';

interface MediaPanelsProps {
  tabs: readonly string[];
  views: Readonly<Record<string, ViewTab>>;
  active: string | null;
  tabElementId: (id: string) => string;
}

export function MediaPanels({ tabs, views, active, tabElementId }: MediaPanelsProps) {
  return (
    <>
      {tabs.map((id) => {
        const view = views[id];
        if (!view) {
          return null;
        }
        const hidden = id !== active;
        return (
          <section
            key={id}
            id={panelElementId(id)}
            role="tabpanel"
            aria-labelledby={tabElementId(id)}
            hidden={hidden}
            className="absolute inset-0 flex min-h-0 flex-col bg-surface"
            data-view={view.kind}
          >
            {view.kind === 'gallery' ? (
              <Gallery hidden={hidden} />
            ) : view.kind === 'mesh' ? (
              <MeshPanel hidden={hidden} />
            ) : (
              <MediaView mediaId={view.mediaId} page={view.page} hidden={hidden} />
            )}
          </section>
        );
      })}
    </>
  );
}
