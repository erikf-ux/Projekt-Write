/**
 * Quitting: everything pending is saved first (section 9: autosave "beim
 * Schließen"). If a save fails, the user decides (the change would be
 * lost otherwise). External files with changes ask "Speichern, Verwerfen,
 * Abbrechen" one by one (ADR 0015).
 */
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';

export async function requestQuit(): Promise<void> {
  const saved = await useWorkspace.getState().flushAll();
  if (!(await useWorkspace.getState().confirmUnsaved())) {
    await call('ui_close_response', { close: false });
    return;
  }
  // Closing the app closes its tabs: versions "beim Schließen" (stage 4).
  await useWorkspace.getState().recordCloseVersions();
  if (saved) {
    await call('ui_close_response', { close: true });
    return;
  }
  useUi.setState({ closePrompt: true });
}

export async function answerClosePrompt(quit: boolean): Promise<void> {
  useUi.setState({ closePrompt: false });
  await call('ui_close_response', { close: quit });
}
