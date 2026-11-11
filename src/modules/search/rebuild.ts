/**
 * "Suchindex neu aufbauen" (section 19): from the settings and the command
 * palette. Rust empties the index and builds it again from the tables and
 * files; afterwards the current search runs again.
 */
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import { useSearch } from './searchStore';

let running = false;

export async function rebuildSearchIndex(): Promise<boolean> {
  if (running) {
    return false;
  }
  running = true;
  useUi.getState().showNotice({ tone: 'info', key: 'search.rebuild.running' });
  try {
    const result = await call('search_rebuild', {});
    if (!result.ok) {
      useUi.getState().showNotice({ tone: 'error', key: 'search.rebuild.failed', error: result.error });
      return false;
    }
    const { indexed, unreadable } = result.value;
    useUi
      .getState()
      .showNotice(
        unreadable > 0
          ? { tone: 'error', key: 'search.rebuild.unreadable', params: { indexed, unreadable } }
          : { tone: 'info', key: 'search.rebuild.done', params: { indexed } },
      );
    await useSearch.getState().refresh();
    return true;
  } finally {
    running = false;
  }
}
