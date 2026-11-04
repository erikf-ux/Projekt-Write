import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLocale } from '@/i18n';
import { useWorkspace, type OpenDocument } from '@/modules/documents/workspaceStore';
import { DocumentEditor } from './DocumentEditor';
import { closeAllSessions, getSession } from './session';

function openDocument(
  id: string,
  format: 'md' | 'txt',
  text: string,
  extra: Partial<OpenDocument> = {},
): OpenDocument {
  return {
    id,
    title: `Titel ${id}`,
    format,
    loadedText: text,
    savedText: text,
    revision: 0,
    state: 'saved',
    error: null,
    losses: [],
    view: format === 'md' ? 'rich' : 'plain',
    viewIssues: [],
    loaded: true,
    readOnly: false,
    ...extra,
  };
}

/** The editor as the workbench shows it: following the store. */
function Harness({ active }: { active: string }) {
  const tabs = useWorkspace((state) => state.tabs);
  const open = useWorkspace((state) => state.open);
  return (
    <DocumentEditor
      tabs={tabs}
      open={open}
      active={active}
      tabElementId={(id) => `tab-${id}`}
      limitLineWidth
      focusMode={false}
      readOnly={false}
      onNewDocument={null}
    />
  );
}

function renderEditor(documents: OpenDocument[], active: string) {
  const open = Object.fromEntries(documents.map((document) => [document.id, document]));
  useWorkspace.setState({ tabs: documents.map((document) => document.id), active, open });
  return render(<Harness active={active} />);
}

describe('DocumentEditor', () => {
  beforeEach(() => {
    useLocale.setState({ locale: 'de' });
  });

  afterEach(() => {
    closeAllSessions();
    useWorkspace.setState({ tabs: [], active: null, open: {} });
  });

  it('shows rich text for Markdown and plain text for .txt, one panel per tab', () => {
    renderEditor(
      [openDocument('a', 'md', '# Titel\n\nText.\n'), openDocument('b', 'txt', 'Zeile\n\tTab\n')],
      'a',
    );
    const surfaces = screen.getAllByTestId('editor');
    expect(surfaces).toHaveLength(2);
    expect(surfaces[0]).toHaveAttribute('aria-label', 'Schreibfläche: Titel a');
    expect(surfaces[0]).toHaveAttribute('data-view', 'rich');
    expect(surfaces[1]).toHaveAttribute('data-view', 'plain');
    expect(getSession('b')?.text()).toBe('Zeile\n\tTab\n');
    expect(screen.getAllByRole('tabpanel', { hidden: true })[1]).not.toBeVisible();
  });

  it('names why a Markdown document opened in the source view', () => {
    renderEditor([openDocument('a', 'md', '**`a`** ~~b\\😀 c~~\n')], 'a');
    expect(useWorkspace.getState().open['a']?.view).toBe('source');
    expect(screen.getByTestId('view-issues')).toHaveTextContent(
      'Absatz 1 käme beim Speichern verändert zurück.',
    );
  });

  it('warns before losses and offers to save anyway or cancel', () => {
    const losses = [{ kind: 'tableMergedCells' as const, block: 0, excerpt: 'Kopf' }];
    renderEditor([openDocument('a', 'md', 'Text\n', { state: 'blocked', losses })], 'a');
    const warning = screen.getByTestId('loss-warning');
    expect(warning).toHaveTextContent('Verbundene Tabellenzellen – „Kopf“');
    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Abbrechen' }));
    });
    expect(useWorkspace.getState().open['a']?.losses).toEqual([]);
  });
});
