import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLocale } from '@/i18n';
import { useWorkspace, type OpenDocument } from '@/modules/documents/workspaceStore';
import { DocumentEditor } from '../DocumentEditor';
import { closeAllSessions, getSession } from '../session';
import { useFind } from './findStore';

const TEXT = 'Der Hund und der hund. Hunde bellen, ein Hund schläft.\n';

function document(id: string, format: 'md' | 'txt'): OpenDocument {
  return {
    id,
    title: id,
    format,
    loadedText: TEXT,
    savedText: TEXT,
    revision: 0,
    state: 'saved',
    error: null,
    losses: [],
    view: format === 'md' ? 'rich' : 'plain',
    viewIssues: [],
    loaded: true,
  };
}

function Harness() {
  const tabs = useWorkspace((state) => state.tabs);
  const open = useWorkspace((state) => state.open);
  const active = useWorkspace((state) => state.active);
  return (
    <DocumentEditor
      tabs={tabs}
      open={open}
      active={active}
      tabElementId={(id) => `tab-${id}`}
      limitLineWidth={false}
      focusMode={false}
      readOnly={false}
      onNewDocument={null}
    />
  );
}

function setup(format: 'md' | 'txt', replace = false) {
  useWorkspace.setState({ tabs: ['a'], active: 'a', open: { a: document('a', format) } });
  render(<Harness />);
  act(() => {
    useFind.getState().open('a', replace);
  });
}

function search(term: string) {
  fireEvent.change(screen.getByTestId('find-term'), { target: { value: term } });
}

describe('find and replace', () => {
  beforeEach(() => {
    useLocale.setState({ locale: 'de' });
  });

  afterEach(() => {
    closeAllSessions();
    useFind.setState({ bars: {} });
    useWorkspace.setState({ tabs: [], active: null, open: {} });
  });

  it.each(['md', 'txt'] as const)(
    'counts results with case, whole words and regular expressions (%s)',
    (format) => {
      setup(format);
      expect(screen.getByTestId('find-term')).toHaveFocus();
      search('hund');
      expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 4');
      fireEvent.click(screen.getByTestId('find-case'));
      expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 1');
      fireEvent.click(screen.getByTestId('find-case'));
      fireEvent.click(screen.getByTestId('find-whole-word'));
      expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 3');
      fireEvent.click(screen.getByTestId('find-regex'));
      search('H.nde?');
      expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 4');
      search('(');
      expect(screen.getByTestId('find-count')).toHaveTextContent('Ungültiger Ausdruck');
      expect(screen.getByTestId('find-term')).toHaveAttribute('aria-invalid', 'true');
      search('Katze');
      expect(screen.getByTestId('find-count')).toHaveTextContent('Keine Treffer');
    },
  );

  it('moves between results with Enter and Shift+Enter', () => {
    setup('md');
    search('Hund');
    const field = screen.getByTestId('find-term');
    // The first result after the cursor is the current one.
    expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 4');
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(screen.getByTestId('find-count')).toHaveTextContent('2 von 4');
    fireEvent.keyDown(field, { key: 'Enter', shiftKey: true });
    expect(screen.getByTestId('find-count')).toHaveTextContent('1 von 4');
    fireEvent.keyDown(field, { key: 'Enter', shiftKey: true });
    expect(screen.getByTestId('find-count')).toHaveTextContent('4 von 4');
    fireEvent.keyDown(field, { key: 'Enter' });
    const { from, to } = getSession('a')?.editor.state.selection ?? { from: 0, to: 0 };
    expect(getSession('a')?.editor.state.doc.textBetween(from, to)).toBe('Hund');
  });

  it('replaces one result or all of them, and closes with Escape', () => {
    setup('md', true);
    search('Hund');
    fireEvent.click(screen.getByTestId('find-whole-word'));
    fireEvent.change(screen.getByTestId('find-replacement'), { target: { value: 'Katze' } });
    fireEvent.click(screen.getByTestId('find-replace'));
    expect(getSession('a')?.text()).toBe('Der Katze und der hund. Hunde bellen, ein Hund schläft.\n');
    fireEvent.click(screen.getByTestId('find-replace-all'));
    expect(getSession('a')?.text()).toBe('Der Katze und der Katze. Hunde bellen, ein Katze schläft.\n');
    // Replacing is an edit like typing: autosave follows.
    expect(useWorkspace.getState().open['a']?.state).toBe('pending');
    fireEvent.keyDown(screen.getByTestId('find-term'), { key: 'Escape' });
    expect(screen.queryByTestId('find-bar')).not.toBeInTheDocument();
    expect(getSession('a')?.searchResults().count).toBe(0);
  });
});
