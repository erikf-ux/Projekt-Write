/**
 * Stage 1, "Sitzung" (ADR 0016, decision 10): the open tabs, the active tab
 * and each tab's view and selection are saved in Rust 2 s after the tabs
 * change and before quitting, and opened again at the next start with the
 * same library. Saving starts only after the restore, so the empty
 * workbench of a start never overwrites the last session.
 */
import { getSession } from '@/modules/editor/session';
import { call } from '@/services/ipc';
import type { SessionTabRequest } from '@/types/generated/SessionTabRequest';
import type { TabView } from '@/types/generated/TabView';
import { externalTabId, useWorkspace } from './workspaceStore';

export const SESSION_SAVE_DELAY_MS = 2000;

interface PendingSelection {
  anchor: number;
  head: number;
  view: TabView;
}

const pending = new Map<string, PendingSelection>();
let tracking = false;
let timer: ReturnType<typeof setTimeout> | null = null;
let unsubscribe: (() => void) | null = null;

/** The selection to put back once the editor of a tab exists. */
export function takePendingSelection(id: string): PendingSelection | undefined {
  const selection = pending.get(id);
  pending.delete(id);
  return selection;
}

function tabRequests(): { tabs: SessionTabRequest[]; active: number | null } {
  const { tabs, open, active } = useWorkspace.getState();
  const requests: SessionTabRequest[] = [];
  let activeIndex: number | null = null;
  for (const id of tabs) {
    const document = open[id];
    if (!document) {
      continue;
    }
    const waiting = pending.get(id);
    let selection = waiting ?? { anchor: 1, head: 1 };
    const session = getSession(id);
    if (session && !waiting) {
      try {
        selection = session.selection();
      } catch {
        // A session without editor keeps the default.
      }
    }
    if (id === active) {
      activeIndex = requests.length;
    }
    requests.push({
      target: document.external
        ? { kind: 'external', id: document.external.fileId }
        : { kind: 'library', id },
      anchor: selection.anchor,
      head: selection.head,
      view: waiting?.view ?? document.view,
    });
  }
  return { tabs: requests, active: activeIndex };
}

/** Saves the session now (before quitting). */
export async function saveSessionNow(): Promise<void> {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  if (!tracking) {
    return;
  }
  await call('session_save', tabRequests());
}

/** Saves 2 s after the tabs or the active tab changed. */
export function startSessionTracking(): void {
  tracking = true;
  if (unsubscribe) {
    return;
  }
  unsubscribe = useWorkspace.subscribe((state, previous) => {
    if (!tracking || (state.tabs === previous.tabs && state.active === previous.active)) {
      return;
    }
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void saveSessionNow();
    }, SESSION_SAVE_DELAY_MS);
  });
}

/** Stops saving (while another library opens). */
export function stopSessionTracking(): void {
  tracking = false;
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
}

/**
 * Opens the tabs of the last session in this library again. A tab the user
 * opens or activates meanwhile (a new document, a file from a second
 * start) stays the active one; the tabs of the session open behind it.
 */
export async function restoreSession(): Promise<void> {
  // `own` is the tab this function is opening; any other activation is
  // the user's choice. Watched from the start, before Rust answers.
  const choice: { own: string | null; chosen: string | null } = { own: null, chosen: null };
  const stopWatching = useWorkspace.subscribe((state, previous) => {
    if (state.active !== previous.active && state.active !== choice.own) {
      choice.chosen = state.active;
    }
  });
  const keepChoice = () => {
    const workspace = useWorkspace.getState();
    const { chosen } = choice;
    if (chosen !== null && workspace.active !== chosen) {
      workspace.activate(chosen);
    }
  };
  try {
    const result = await call('session_restore', {});
    if (!result.ok || result.value.tabs.length === 0) {
      return;
    }
    const workspace = useWorkspace.getState();
    const ids: string[] = [];
    for (const tab of result.value.tabs) {
      const id = tab.target.kind === 'library' ? tab.target.id : externalTabId(tab.target.file.id);
      pending.set(id, { anchor: tab.anchor, head: tab.head, view: tab.view });
      choice.own = id;
      const opened =
        tab.target.kind === 'library'
          ? await workspace.openDocument(tab.target.id)
          : await workspace.openExternal(tab.target.file);
      choice.own = null;
      keepChoice();
      if (opened) {
        ids.push(id);
      } else {
        pending.delete(id);
      }
    }
    const active = result.value.active === null ? undefined : result.value.tabs[result.value.active];
    if (choice.chosen === null && active) {
      const id = active.target.kind === 'library' ? active.target.id : externalTabId(active.target.file.id);
      if (ids.includes(id)) {
        choice.own = id;
        useWorkspace.getState().activate(id);
      }
    }
  } finally {
    stopWatching();
  }
}
