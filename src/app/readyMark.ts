/**
 * "Kaltstart bis zur Eingabemöglichkeit" (section 6): the workbench marks
 * the first frame in which it takes input. By then the library is open,
 * the Explorer stands, the tabs of the last session are back and the
 * writing surface of the active tab is editable (a tab without a writing
 * surface, or no tab, counts once its panel or the Explorer is shown).
 *
 * The E2E test measures from the start of the process to this mark
 * (tests/e2e/processes.ts). The mark is set once per page load, like the
 * marks of the Explorer measurement (ADR 0017, decision 17).
 */
import { EXPLORER_READY_MARK } from '@/modules/documents/Explorer';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { panelElementId } from '@/modules/editor/DocumentEditor';

export const READY_FOR_INPUT_MARK = 'pw:ready-for-input';

/** About 10 s at 60 frames a second; a start that takes longer is not marked. */
const MAX_FRAMES = 600;

function marked(): boolean {
  return performance.getEntriesByName(READY_FOR_INPUT_MARK).length > 0;
}

/** Whether the Explorer and the active tab take input in this frame. */
function readyForInput(): boolean {
  if (performance.getEntriesByName(EXPLORER_READY_MARK).length === 0) {
    return false;
  }
  const { active, open } = useWorkspace.getState();
  if (active === null) {
    return true;
  }
  const panel = document.getElementById(panelElementId(active));
  if (!panel || panel.hidden) {
    return false;
  }
  // A document tab takes input once its writing surface is editable; other
  // tabs (gallery, media) once their panel is shown.
  if (!(active in open)) {
    return true;
  }
  return [...panel.querySelectorAll<HTMLElement>('[data-testid="editor"]')].some(
    (surface) => surface.isContentEditable && surface.getClientRects().length > 0,
  );
}

/** Sets the mark after the first drawn frame in which the workbench takes input. */
export function markReadyForInput(): void {
  if (marked()) {
    return;
  }
  let frames = 0;
  const check = () => {
    frames += 1;
    if (marked() || frames > MAX_FRAMES) {
      return;
    }
    if (!readyForInput()) {
      requestAnimationFrame(check);
      return;
    }
    // Checked before the frame is drawn; a message posted now runs after it.
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      if (!marked()) {
        performance.mark(READY_FOR_INPUT_MARK);
      }
    };
    channel.port2.postMessage(null);
  };
  requestAnimationFrame(check);
}
