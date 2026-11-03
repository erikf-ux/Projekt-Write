/** Moving the keyboard focus between the main regions. */
import { EXPLORER_LIST_ID } from '@/modules/documents/Explorer';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { editorElementId } from '@/modules/editor/PlainTextEditor';

/** After the next render (the region may just become visible). */
function later(action: () => void): void {
  window.setTimeout(action, 0);
}

export function focusExplorer(): void {
  later(() => {
    document.getElementById(EXPLORER_LIST_ID)?.focus();
  });
}

export function focusEditor(): void {
  later(() => {
    const id = useWorkspace.getState().active;
    if (id) {
      document.getElementById(editorElementId(id))?.focus();
    }
  });
}
