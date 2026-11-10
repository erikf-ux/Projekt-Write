import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { act, createEvent, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { usePrompt } from '@/components/prompt/promptStore';
import { useLocale } from '@/i18n';
import { Explorer, EXPLORER_DRAG_TYPE } from '@/modules/documents/Explorer';
import { useExplorer } from '@/modules/documents/explorerStore';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useUi } from '@/modules/settings/uiStore';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { StructureUpdate } from '@/types/generated/StructureUpdate';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';
import { Breadcrumbs } from './Breadcrumbs';
import { useStructureDialogs } from './dialogStore';
import { SubnetDialog } from './SubnetDialog';
import { SubnetPicker } from './SubnetPicker';
import { useStructure } from './structureStore';
import { buildIndex, lowestContrast, normalizeHex, pathLabel, subtree } from './tree';

const P = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b31';
const R = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b32';
const W = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b33';
const DOC = '2f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

function subnet(id: string, name: string, parentId: string | null = null): SubnetSummary {
  return {
    id,
    name,
    description: '',
    color: 'green',
    parentId,
    createdAt: '2026-11-05T10:00:00.000Z',
    updatedAt: '2026-11-05T10:00:00.000Z',
  };
}

function doc(id: string, title: string): DocumentSummary {
  return {
    id,
    title,
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-05T10:00:00.000Z',
    updatedAt: '2026-11-05T10:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
  };
}

const SUBNETS = [subnet(P, 'Projekte'), subnet(R, 'Roman', P), subnet(W, 'Wissen')];

function update(extra: Partial<StructureUpdate> = {}): StructureUpdate {
  return {
    subnets: SUBNETS,
    tags: [],
    documents: [],
    history: { undo: null, redo: null },
    step: null,
    subjectId: null,
    ...extra,
  };
}

const calls: { command: string; request: Record<string, unknown> }[] = [];
const initialWorkspace = useWorkspace.getState();

beforeEach(() => {
  calls.length = 0;
  useLocale.setState({ locale: 'de' });
  mockIPC((command, args) => {
    const request = (args as { request?: Record<string, unknown> } | undefined)?.request ?? {};
    calls.push({ command, request });
    switch (command) {
      case 'structure_state':
        return {
          subnets: SUBNETS,
          tags: [{ id: 't1', name: 'Entwurf', createdAt: '', updatedAt: '' }],
          documents: [{ id: DOC, subnetIds: [W, R], tagIds: ['t1'] }],
          history: { undo: null, redo: null },
        };
      case 'subnet_create':
        return update({
          step: { action: 'subnetCreate', subject: String(request['name']) },
          history: { undo: { action: 'subnetCreate', subject: String(request['name']) }, redo: null },
          subjectId: 'new',
        });
      case 'structure_undo':
        return update({
          step: { action: 'subnetCreate', subject: 'Neu' },
          history: { undo: null, redo: { action: 'subnetCreate', subject: 'Neu' } },
        });
      case 'subnet_move':
        if (request['parentId'] === R) {
          throw { code: 'SUBNET_CYCLE', messageKey: 'error.subnetCycle' };
        }
        return update({ step: { action: 'subnetMove', subject: 'Projekte' } });
      case 'document_move':
      case 'document_assign':
      case 'subnet_delete':
      case 'tag_merge':
        return update({ step: { action: 'documentMove', subject: 'Kapitel' } });
      case 'tag_rename':
        throw { code: 'TAG_NAME_TAKEN', messageKey: 'error.tagNameTaken', details: { existing: 't1' } };
      default:
        return null;
    }
  });
});

afterEach(() => {
  clearMocks();
  useStructure.getState().reset();
  useStructureDialogs.getState().close();
  useExplorer.getState().reset();
  useWorkspace.setState(initialWorkspace, true);
  useUi.setState({ notice: null });
  usePrompt.setState({ queue: [] });
});

describe('subnet tree helpers', () => {
  it('knows paths, subtrees and colours that reach 3:1 everywhere', () => {
    const index = buildIndex(SUBNETS, 'de');
    expect(pathLabel(index, R)).toBe('Projekte › Roman');
    expect([...subtree(index, P)].sort()).toEqual([P, R].sort());
    expect(normalizeHex(' #D9480F ')).toBe('#d9480f');
    expect(normalizeHex('red')).toBeNull();
    expect(lowestContrast('#d9480f')).toBeGreaterThanOrEqual(3);
    expect(lowestContrast('#ffffff')).toBeLessThan(3);
  });
});

describe('structure store', () => {
  it('takes over what Rust answers and offers to undo every step', async () => {
    await useStructure.getState().load();
    expect(useStructure.getState().byDocument[DOC]?.subnetIds).toEqual([W, R]);
    const result = await useStructure.getState().createSubnet({ name: 'Neu' });
    expect(result).toEqual({ id: 'new', error: null });
    const notice = useUi.getState().notice;
    expect(notice?.params?.['what']).toBe('Subnetz „Neu“ angelegt');
    expect(notice?.action?.labelKey).toBe('structure.undo');
    act(() => {
      notice?.action?.run();
    });
    await waitFor(() => {
      expect(calls.map((call) => call.command)).toContain('structure_undo');
    });
    await waitFor(() => {
      expect(useUi.getState().notice?.key).toBe('structure.undone');
    });
    expect(useStructure.getState().history.redo?.subject).toBe('Neu');
  });

  it('asks before deleting a subnet and passes the choice for its subnets', async () => {
    await useStructure.getState().load();
    const deleting = useStructure.getState().deleteSubnet(P);
    await waitFor(() => {
      expect(usePrompt.getState().queue).toHaveLength(1);
    });
    expect(usePrompt.getState().queue[0]?.request.params).toMatchObject({ name: 'Projekte', children: 1 });
    usePrompt.getState().answer('withChildren');
    await deleting;
    expect(calls.find((call) => call.command === 'subnet_delete')?.request).toEqual({
      id: P,
      withChildren: true,
    });
  });

  it('offers to merge when a tag is renamed to an existing name', async () => {
    await useStructure.getState().load();
    useStructure.setState({
      tags: [
        { id: 't1', name: 'Entwurf', createdAt: '', updatedAt: '' },
        { id: 't2', name: 'Skizze', createdAt: '', updatedAt: '' },
      ],
    });
    const renaming = useStructure.getState().renameTag('t2', 'entwurf');
    await waitFor(() => {
      expect(usePrompt.getState().queue).toHaveLength(1);
    });
    expect(usePrompt.getState().queue[0]?.request.params).toEqual({ source: 'Skizze', target: 'Entwurf' });
    usePrompt.getState().answer('merge');
    await renaming;
    expect(calls.find((call) => call.command === 'tag_merge')?.request).toEqual({
      sourceId: 't2',
      targetId: 't1',
    });
  });
});

describe('subnet dialog', () => {
  it('accepts an own colour only with 3:1 contrast in both designs', async () => {
    await useStructure.getState().load();
    useStructureDialogs.getState().openSubnet({ mode: 'create', parentId: null });
    render(<SubnetDialog />);
    fireEvent.change(screen.getByTestId('subnet-name'), { target: { value: 'Eigen' } });
    fireEvent.click(screen.getByRole('radio', { name: 'Eigene Farbe' }));
    fireEvent.change(screen.getByTestId('subnet-color-hex'), { target: { value: '#fafafa' } });
    expect(screen.getByTestId('subnet-color-contrast').textContent).toContain('Zu wenig Kontrast');
    expect(screen.getByTestId<HTMLButtonElement>('subnet-save').disabled).toBe(true);
    fireEvent.change(screen.getByTestId('subnet-color-hex'), { target: { value: '#D9480F' } });
    expect(screen.getByTestId<HTMLButtonElement>('subnet-save').disabled).toBe(false);
    fireEvent.click(screen.getByTestId('subnet-save'));
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'subnet_create')?.request).toEqual({
        name: 'Eigen',
        color: '#d9480f',
        description: '',
      });
    });
  });
});

describe('subnet picker', () => {
  it('never offers a subnet below itself and moves to the chosen one', async () => {
    await useStructure.getState().load();
    useStructureDialogs.getState().openPicker({ mode: 'moveSubnet', subnetId: P });
    render(<SubnetPicker />);
    const list = screen.getByRole('listbox');
    const choices = [...list.querySelectorAll('[data-subnet-choice]')].map((entry) => entry.textContent);
    expect(choices).toEqual(['Oberste Ebene', 'Wissen']);
    fireEvent.click(within(list).getByText('Wissen'));
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'subnet_move')?.request).toEqual({ id: P, parentId: W });
    });
  });

  it('marks the primary subnet and moves a document with the keyboard', async () => {
    await useStructure.getState().load();
    useWorkspace.setState({ documents: [doc(DOC, 'Kapitel')] });
    useStructureDialogs.getState().openPicker({ mode: 'moveDocument', documentId: DOC });
    render(<SubnetPicker />);
    expect(screen.getByRole('option', { name: /Wissen/ }).textContent).toContain('primär');
    const search = screen.getByTestId('subnet-picker-search');
    fireEvent.change(search, { target: { value: 'roman' } });
    fireEvent.keyDown(search, { key: 'Enter' });
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'document_move')?.request).toEqual({
        id: DOC,
        subnetId: R,
      });
    });
  });
});

describe('breadcrumbs', () => {
  it('show the path of the primary subnet, the others and the tags', async () => {
    await useStructure.getState().load();
    render(<Breadcrumbs documentId={DOC} title="Kapitel" />);
    const nav = screen.getByRole('navigation', { name: 'Pfad des Dokuments' });
    expect(nav.textContent).toContain('Wissen›Kapitel');
    expect(screen.getByTestId('breadcrumbs-others').textContent).toContain('Roman');
    expect(screen.getByTestId('breadcrumbs-tags').textContent).toContain('#Entwurf');
    fireEvent.click(screen.getByText('Roman'));
    expect(useExplorer.getState().reveal?.key).toBe(`s:${R}`);
    expect(useExplorer.getState().expanded.has(P)).toBe(true);
  });
});

describe('Explorer with subnets', () => {
  function transfer(data: Record<string, string>) {
    return {
      types: Object.keys(data),
      getData: (type: string) => data[type] ?? '',
      setData: () => undefined,
      dropEffect: 'move',
      effectAllowed: 'all',
    };
  }

  it('moves and assigns by drag and drop and undoes with Ctrl+Z', async () => {
    await useStructure.getState().load();
    useWorkspace.setState({ documents: [doc(DOC, 'Kapitel')], listLoaded: true });
    useExplorer.getState().expandPath([P, R, W]);
    render(<Explorer onNewDocument={() => undefined} />);
    const tree = screen.getByRole('tree');
    // One document, two places; the further one carries a mark.
    const rows = tree.querySelectorAll(`[data-document="${DOC}"]`);
    expect(rows).toHaveLength(2);
    expect(tree.querySelector(`[data-subnet-member="${R}"][data-secondary]`)).not.toBeNull();

    const projects = tree.querySelector(`[data-subnet="${P}"]`) as HTMLElement;
    const data = transfer({ [EXPLORER_DRAG_TYPE]: JSON.stringify({ kind: 'document', id: DOC }) });
    fireEvent.dragOver(projects, { dataTransfer: data });
    fireEvent.drop(projects, { dataTransfer: data });
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'document_move')?.request).toEqual({
        id: DOC,
        subnetId: P,
      });
    });
    // jsdom has no DragEvent: the Ctrl key is set on the event by hand.
    const withControl = createEvent.drop(projects, { dataTransfer: data });
    Object.defineProperty(withControl, 'ctrlKey', { value: true });
    fireEvent(projects, withControl);
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'document_assign')?.request).toEqual({
        id: DOC,
        subnetId: P,
      });
    });
    // A subnet onto a subnet becomes its child (Rust refuses cycles).
    const wissen = tree.querySelector(`[data-subnet="${W}"]`) as HTMLElement;
    fireEvent.drop(wissen, {
      dataTransfer: transfer({ [EXPLORER_DRAG_TYPE]: JSON.stringify({ kind: 'subnet', id: P }) }),
    });
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'subnet_move')?.request).toEqual({ id: P, parentId: W });
    });
    // Into its own subnet: Rust refuses, and the Explorer says so (F-29).
    const roman = tree.querySelector(`[data-subnet="${R}"]`) as HTMLElement;
    fireEvent.drop(roman, {
      dataTransfer: transfer({ [EXPLORER_DRAG_TYPE]: JSON.stringify({ kind: 'subnet', id: P }) }),
    });
    await waitFor(() => {
      expect(useUi.getState().notice?.error?.code).toBe('SUBNET_CYCLE');
    });
    // Files from outside are not Explorer items.
    const before = calls.length;
    fireEvent.drop(wissen, { dataTransfer: transfer({ Files: '' }) });
    expect(calls.length).toBe(before);

    fireEvent.keyDown(tree, { key: 'z', ctrlKey: true });
    await waitFor(() => {
      expect(calls.map((call) => call.command)).toContain('structure_undo');
    });
  });
});
