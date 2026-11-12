/**
 * The Mesh in the running app (Phase 11, section 15, ADR 0022): views,
 * keyboard operation (acceptance: "Das Mesh ist auch per Tastatur
 * bedienbar"), connections with undo, the automatic layout in its web
 * worker leaving pinned nodes alone, and suggestions that change nothing
 * until accepted; a rejected one does not come back. The data comparison
 * after a run is in Rust (tests/mesh.rs). Runs after search.e2e.ts; closes
 * the Mesh tab again so the session files after it see the tabs they
 * expect.
 */
import { $, browser, expect } from '@wdio/globals';
import { runFromPalette } from '../editing';
import { pinMainWindow } from '../session';

interface Graph {
  view: string;
  nodes: { id: string; kind: string; title: string; x: number | null; y: number | null; pinned: boolean }[];
  edges: { id: string; source: string; target: string; kind: string; connectionId: string | null }[];
}

async function invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
  return browser.execute(
    async (name: string, body: Record<string, unknown>) => {
      const internals = (
        window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }
      ).__TAURI_INTERNALS__;
      return internals.invoke(name, { request: body });
    },
    command,
    request,
  ) as Promise<T>;
}

async function announced(): Promise<string> {
  return $('[data-testid="mesh-announce"]').getText();
}

async function selectedNode(): Promise<string> {
  return browser.execute(
    () =>
      document.querySelector('[data-testid="mesh"] [data-selected="true"]')?.getAttribute('data-mesh-node') ??
      '',
  );
}

const TEXT =
  'Der Leuchtturm steht auf der Klippe. Sein Licht dreht sich jede Nacht über das Meer, Schiffe sehen den Leuchtturm von weitem.';

describe('Mesh (Phase 11)', () => {
  let subnet = '';
  let nord = '';
  let sued = '';
  let ost = '';

  before(async () => {
    await pinMainWindow();
    const created = await invoke<{ subjectId: string }>('subnet_create', { name: 'Küste' });
    subnet = created.subjectId;
    for (const [title, set] of [
      ['Leuchtturm Nord', (id: string) => (nord = id)],
      ['Leuchtturm Süd', (id: string) => (sued = id)],
      ['Leuchtturm Ost', (id: string) => (ost = id)],
    ] as const) {
      const document = await invoke<{ id: string }>('document_create', { title, format: 'md' });
      await invoke('document_save', { id: document.id, text: `${TEXT}\n` });
      await invoke('document_assign', { id: document.id, subnetId: subnet });
      await invoke('document_tag', { id: document.id, name: 'Licht' });
      set(document.id);
    }
  });

  it('opens with Ctrl+Shift+M and moves through the views by keyboard', async () => {
    await browser.keys(['Control', 'Shift', 'm']);
    await $(`[data-testid="mesh"] [data-mesh-node="${subnet}"]`).waitForDisplayed({
      timeoutMsg: 'the new subnet is not in the Global View',
    });
    const canvas = $('[data-testid="mesh-canvas"]');
    await canvas.click();
    // Arrows select a node and say what it is.
    await browser.keys('ArrowRight');
    await browser.waitUntil(async () => (await selectedNode()) !== '', {
      timeoutMsg: 'no node selected by keyboard',
    });
    expect(await announced()).not.toBe('');
    // Shift+click selects the subnet without opening it; Enter shows it
    // (Subnet View).
    // The view loads twice right after opening (graph, then fresh
    // structure): a click in between may land on a node being replaced.
    for (
      let attempt = 0;
      attempt < 5 && !(await $('[data-testid="mesh-breadcrumb"]').getText()).includes('Küste');
      attempt += 1
    ) {
      await $(`[data-testid="mesh"] [data-mesh-node="${subnet}"]`).click();
      await browser.pause(400);
    }
    expect(await $('[data-testid="mesh-breadcrumb"]').getText()).toContain('Küste');
    await canvas.click();
    await browser.keys('Escape');
    await browser.waitUntil(
      async () => !(await $('[data-testid="mesh-breadcrumb"]').getText()).includes('Küste'),
    );
    await browser.execute((id: string) => {
      const node = document.querySelector<HTMLElement>(`[data-testid="mesh"] [data-mesh-node="${id}"]`);
      node?.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true }));
    }, subnet);
    await browser.waitUntil(async () => (await selectedNode()) === subnet, {
      timeoutMsg: 'Shift+click selected nothing',
    });
    await canvas.click();
    await browser.keys('Enter');
    await expect($('[data-testid="mesh-breadcrumb"]')).toHaveText(expect.stringContaining('Küste'));
    await $(`[data-testid="mesh"] [data-mesh-node="${nord}"]`).waitForDisplayed();
    // Escape goes back to the Global View.
    await canvas.click();
    await browser.keys('Escape');
    await browser.waitUntil(
      async () => !(await $('[data-testid="mesh-breadcrumb"]').getText()).includes('Küste'),
      { timeoutMsg: 'Escape did not go back' },
    );
  });

  it('connects two documents by keyboard, removes the connection with Delete and undoes it', async () => {
    // Focus View of "Leuchtturm Nord" (F), from the Subnet View.
    await $(`[data-testid="mesh"] [data-mesh-node="${subnet}"]`).click();
    await $(`[data-testid="mesh"] [data-mesh-node="${nord}"]`).waitForDisplayed();
    const canvas = $('[data-testid="mesh-canvas"]');
    await canvas.click();
    // As a person would: look where the node is and press that arrow.
    const reach = async (id: string) => {
      for (let step = 0; step < 12 && (await selectedNode()) !== id; step += 1) {
        const key = await browser.execute(
          (from: string, to: string) => {
            const center = (node: string) => {
              const box = document
                .querySelector(`[data-testid="mesh"] [data-mesh-node="${node}"]`)
                ?.getBoundingClientRect();
              return box ? { x: box.left + box.width / 2, y: box.top + box.height / 2 } : { x: 0, y: 0 };
            };
            const a = center(from);
            const b = center(to);
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            if (Math.abs(dx) >= Math.abs(dy)) {
              return dx > 0 ? 'ArrowRight' : 'ArrowLeft';
            }
            return dy > 0 ? 'ArrowDown' : 'ArrowUp';
          },
          await selectedNode(),
          id,
        );
        await browser.keys(key);
      }
    };
    await browser.keys('ArrowRight');
    await reach(nord);
    expect(await selectedNode()).toBe(nord);
    // C starts a connection; arrows choose the other end; Enter connects.
    await browser.keys('c');
    await $('[data-testid="mesh-connecting"]').waitForDisplayed();
    await reach(sued);
    expect(await selectedNode()).toBe(sued);
    await browser.keys('Enter');
    const connected = async () => {
      const graph = await invoke<Graph>('mesh_graph', { view: 'focus', scopeId: nord });
      return graph.edges.find((edge) => edge.kind === 'manual' && [edge.source, edge.target].includes(sued));
    };
    await browser.waitUntil(async () => (await connected()) !== undefined, { timeoutMsg: 'not connected' });

    // E selects a connection of the node; Delete removes a confirmed one.
    let found = false;
    for (let step = 0; step < 6 && !found; step += 1) {
      await browser.keys('e');
      found = (await announced()).includes('Verbindung') || (await announced()).includes('Connection');
    }
    expect(found).toBe(true);
    await browser.keys('Delete');
    await browser.waitUntil(async () => (await connected()) === undefined, {
      timeoutMsg: 'Delete removed nothing',
    });
    // Ctrl+Z brings it back with its id.
    await browser.keys(['Control', 'z']);
    await browser.waitUntil(async () => (await connected()) !== undefined, {
      timeoutMsg: 'undo restored nothing',
    });
    // Shift+F10 opens the context menu of the selection; Escape closes it.
    await browser.keys(['Shift', 'F10']);
    const menu = $('[role="menu"]');
    await menu.waitForDisplayed({ timeoutMsg: 'no context menu by keyboard' });
    expect(await menu.getText()).toMatch(/Fokusansicht|Focus View|Verbindung entfernen|Remove connection/);
    await browser.keys('Escape');
    await menu.waitForExist({ reverse: true });
  });

  it('arranges the view in the web worker and leaves a pinned node where it is', async () => {
    await invoke('mesh_save_positions', {
      view: 'subnet',
      scopeId: subnet,
      positions: [{ nodeId: nord, x: 4321, y: 1234, pinned: true }],
    });
    await $('[data-testid="mesh-canvas"]').click();
    await browser.keys('0');
    await $('[data-testid="mesh-layout"]').click();
    await browser.waitUntil(
      async () => {
        const graph = await invoke<Graph>('mesh_graph', { view: 'subnet', scopeId: subnet });
        return graph.nodes.find((node) => node.id === sued)?.x !== null;
      },
      { timeout: 20_000, timeoutMsg: 'the automatic layout placed nothing' },
    );
    const graph = await invoke<Graph>('mesh_graph', { view: 'subnet', scopeId: subnet });
    const pinned = graph.nodes.find((node) => node.id === nord);
    expect(pinned).toMatchObject({ x: 4321, y: 1234, pinned: true });
  });

  it('suggests similar documents; nothing changes until accepted, a rejected one stays away', async () => {
    const before = await invoke<Graph>('mesh_graph', { view: 'focus', scopeId: sued });
    await $('[data-testid="mesh-suggestions-toggle"]').click();
    await $('[data-testid="mesh-suggestions"]').waitForDisplayed();
    await $('[data-testid="mesh-analyze"]').click();
    // Nord and Süd are connected (above): never suggested. Süd and Ost are.
    const item = $(`[data-testid="mesh-suggestion-list"] li[data-nodes~="${sued}"][data-nodes~="${ost}"]`);
    await item.waitForDisplayed({
      timeout: 30_000,
      timeoutMsg: 'the similar lighthouses were not suggested',
    });
    expect(
      await $(
        `[data-testid="mesh-suggestion-list"] li[data-nodes~="${nord}"][data-nodes~="${sued}"]`,
      ).isExisting(),
    ).toBe(false);
    expect(await item.getText()).toMatch(/Tags|Inhalt|tags|content/);
    // The run changed no connection.
    const after = await invoke<Graph>('mesh_graph', { view: 'focus', scopeId: sued });
    expect(
      after.edges
        .filter((edge) => edge.connectionId !== null)
        .map((edge) => edge.id)
        .sort(),
    ).toEqual(
      before.edges
        .filter((edge) => edge.connectionId !== null)
        .map((edge) => edge.id)
        .sort(),
    );
    // Rejected: gone, and a second run does not bring it back.
    await item.$('[data-testid="suggestion-reject"]').click();
    await item.waitForExist({ reverse: true });
    await $('[data-testid="mesh-analyze"]').click();
    await $('[data-testid="mesh-analyze"]').waitForEnabled({ timeout: 30_000 });
    await browser.pause(300);
    expect(await item.isExisting()).toBe(false);
    const graph = await invoke<Graph>('mesh_graph', { view: 'focus', scopeId: sued });
    expect(graph.edges.some((edge) => edge.kind === 'similarity')).toBe(false);
    // Accepted: a connection of its own kind.
    const next = $(`[data-testid="mesh-suggestion-list"] li[data-nodes~="${nord}"][data-nodes~="${ost}"]`);
    await next.waitForDisplayed({ timeoutMsg: 'Nord and Ost were not suggested' });
    await next.$('[data-testid="suggestion-accept"]').click();
    await browser.waitUntil(
      async () =>
        (await invoke<Graph>('mesh_graph', { view: 'focus', scopeId: ost })).edges.some(
          (edge) => edge.kind === 'similarity',
        ),
      { timeoutMsg: 'the accepted suggestion made no connection' },
    );
  });

  it('highlights the elements with a tag and dims the others (tags make no edges)', async () => {
    const other = await invoke<{ id: string }>('document_create', { title: 'Hafenkran', format: 'md' });
    await invoke('document_assign', { id: other.id, subnetId: subnet });
    await $('[data-testid="mesh-breadcrumb"] button').click();
    await $(`[data-testid="mesh"] [data-mesh-node="${subnet}"]`).click();
    await $(`[data-testid="mesh"] [data-mesh-node="${other.id}"]`).waitForDisplayed();
    // Shown anew (another tab, then back), the Mesh reads subnets and tags again.
    await runFromPalette('media.gallery');
    await browser.keys(['Control', 'Shift', 'm']);
    await browser.waitUntil(
      () =>
        browser.execute(() =>
          [...document.querySelectorAll('[data-testid="mesh-tag"] option')].some(
            (option) => option.textContent === 'Licht',
          ),
        ),
      { timeoutMsg: 'the tag is not offered' },
    );
    const tag = await browser.execute(
      () =>
        [...document.querySelectorAll<HTMLOptionElement>('[data-testid="mesh-tag"] option')].find(
          (option) => option.textContent === 'Licht',
        )?.value,
    );
    expect(tag).toBeDefined();
    const choose = (value: string) =>
      browser.execute((wanted: string) => {
        const select = document.querySelector<HTMLSelectElement>('[data-testid="mesh-tag"]');
        if (select) {
          select.value = wanted;
          select.dispatchEvent(new Event('change', { bubbles: true }));
        }
      }, value);
    await choose(tag ?? '');
    const opacity = (id: string) =>
      browser.execute(
        (node: string) =>
          document.querySelector<HTMLElement>(`.react-flow__node[data-id="${node}"]`)?.style.opacity ?? '',
        id,
      );
    await browser.waitUntil(async () => (await opacity(other.id)) === '0.25', {
      timeoutMsg: 'nothing dimmed',
    });
    expect(await opacity(nord)).toBe('1');
    // Süd and Ost share the tag (their suggestion was rejected above): no
    // edge between them. Tags make no edges.
    const graph = await invoke<Graph>('mesh_graph', { view: 'subnet', scopeId: subnet });
    expect(
      graph.edges.some(
        (edge) => [edge.source, edge.target].includes(sued) && [edge.source, edge.target].includes(ost),
      ),
    ).toBe(false);
    await choose('');
  });

  after(async () => {
    // Close the Mesh tab: the session files after this one count the tabs.
    await browser.execute(() => {
      document.querySelector<HTMLElement>('[data-testid="tabs"] [data-tab="view:mesh"] button')?.click();
    });
  });
});
