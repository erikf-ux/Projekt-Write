import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runCommand, registerCommands, findCommand } from '@/modules/commands/registry';
import { useWorkspace, type OpenDocument } from '@/modules/documents/workspaceStore';
import { isAllowedLink, useLinkDialog } from '@/modules/editor/LinkDialog';
import { closeAllSessions, getSession, linkAt, openSession } from '@/modules/editor/session';
import { useLibrary } from '@/modules/library/libraryStore';
import { createFormatCommands } from './formatCommands';

function document(id: string, format: 'md' | 'txt', view: OpenDocument['view']): OpenDocument {
  return {
    id,
    title: id,
    format,
    loadedText: '',
    savedText: '',
    revision: 0,
    state: 'saved',
    error: null,
    losses: [],
    view,
    viewIssues: [],
    loaded: true,
  };
}

function open(id: string, format: 'md' | 'txt', text: string, onOpenLink?: (href: string) => void) {
  const session = openSession({
    id,
    format,
    text,
    label: id,
    editable: true,
    onChange: () => undefined,
    onBlur: () => undefined,
    ...(onOpenLink ? { onOpenLink } : {}),
  });
  window.document.body.append(session.root);
  useWorkspace.setState({
    tabs: [id],
    active: id,
    open: { [id]: document(id, format, session.isMarkdown ? session.view : 'plain') },
  });
  return session;
}

describe('format commands', () => {
  beforeEach(() => {
    registerCommands(createFormatCommands());
    useLibrary.setState({ status: { state: 'open' } as never });
  });

  afterEach(() => {
    closeAllSessions();
    useWorkspace.setState({ tabs: [], active: null, open: {} });
    useLinkDialog.setState({ href: null });
  });

  it('formats the selection of rich text', async () => {
    const session = open('a', 'md', 'Ein Wort hier.\n');
    session.editor.commands.setTextSelection({ from: 5, to: 9 });
    await runCommand('format.bold');
    expect(session.text()).toBe('Ein **Wort** hier.\n');
    await runCommand('format.heading2');
    expect(session.text()).toBe('## Ein **Wort** hier.\n');
    await runCommand('format.bulletList');
    expect(session.text()).toBe('- Ein **Wort** hier.\n');
    // Block changes in quick succession form one undo step (ProseMirror history).
    await runCommand('edit.undo');
    expect(session.text()).toBe('Ein **Wort** hier.\n');
    await runCommand('edit.redo');
    expect(session.text()).toBe('- Ein **Wort** hier.\n');
  });

  it('offers formatting only in rich text', () => {
    open('t', 'txt', 'Reiner Text\n');
    expect(findCommand('format.bold')?.isAvailable()).toBe(false);
    expect(findCommand('edit.undo')?.isAvailable()).toBe(true);
    closeAllSessions();
    const markdown = open('m', 'md', 'Text\n');
    expect(findCommand('format.bold')?.isAvailable()).toBe(true);
    markdown.toggleView();
    useWorkspace.setState({ open: { m: document('m', 'md', 'source') } });
    expect(findCommand('format.bold')?.isAvailable()).toBe(false);
  });

  it('sets links through the dialog, only for allowed addresses', async () => {
    const session = open('a', 'md', 'Siehe hier.\n');
    session.editor.commands.setTextSelection({ from: 7, to: 11 });
    await runCommand('format.link');
    expect(useLinkDialog.getState().href).toBe('');
    session.editor.chain().setLink({ href: 'https://example.org' }).run();
    expect(session.text()).toBe('Siehe [hier](https://example.org).\n');
    expect(linkAt(session.editor.state.doc, 8)).toBe('https://example.org');
    expect(linkAt(session.editor.state.doc, 2)).toBeNull();
    for (const href of ['https://example.org', 'mailto:a@example.org', 'kapitel-2.md', '#teil']) {
      expect(isAllowedLink(href), href).toBe(true);
    }
    for (const href of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,x', 'mit leer', '']) {
      expect(isAllowedLink(href), href).toBe(false);
    }
  });

  it('opens a web link on Ctrl+click and never lets the webview follow it', () => {
    const opened = vi.fn();
    const session = open('a', 'md', '[Link](https://example.org) dahinter\n', opened);
    const view = session.editor.view;
    const click = (ctrlKey: boolean) =>
      view.someProp('handleClick', (handler) => handler(view, 3, new MouseEvent('click', { ctrlKey })));
    expect(click(false)).toBeFalsy();
    expect(opened).not.toHaveBeenCalled();
    expect(click(true)).toBe(true);
    expect(opened).toHaveBeenCalledWith('https://example.org');
    const anchor = view.dom.querySelector('a');
    const event = new MouseEvent('click', { bubbles: true, cancelable: true });
    anchor?.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(getSession('a')).toBe(session);
  });
});
