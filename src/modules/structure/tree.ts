/**
 * The subnet tree as the interface uses it (section 16; ADR 0017): an index
 * over the list Rust sends, paths for breadcrumbs and pickers, and the
 * colours. Pure functions; the store keeps the data.
 */
import { themes } from '@/modules/themes/tokens';
import { contrastRatio } from '@/modules/themes/contrast';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';

/** The cluster colours of section 11, stored by token name. */
export const CLUSTER_COLORS = ['blue', 'green', 'violet', 'orange', 'grey'] as const;
export type ClusterColor = (typeof CLUSTER_COLORS)[number];
/** Section 6: graphical elements reach 3:1. */
export const MIN_COLOR_CONTRAST = 3;
/** Separator of a path, as shown. */
export const PATH_SEPARATOR = ' › ';

export interface SubnetIndex {
  byId: ReadonlyMap<string, SubnetSummary>;
  /** Children of a subnet, by name; `null` holds the top level. */
  children: ReadonlyMap<string | null, readonly SubnetSummary[]>;
}

export function isClusterColor(color: string): color is ClusterColor {
  return (CLUSTER_COLORS as readonly string[]).includes(color);
}

/** CSS value of a subnet colour: a cluster token follows the design. */
export function colorValue(color: string): string {
  return isClusterColor(color) ? `var(--pw-color-cluster-${color})` : color;
}

/**
 * Contrast of a custom colour against writing surface and side bar of both
 * designs; the lowest one decides (Rust checks the same, ADR 0017).
 */
export function lowestContrast(color: string): number {
  const backgrounds = [themes.dark.surface, themes.dark.sidebar, themes.light.surface, themes.light.sidebar];
  return Math.min(...backgrounds.map((background) => contrastRatio(color, background)));
}

/** `#rrggbb`, or null. */
export function normalizeHex(value: string): string | null {
  const text = value.trim().toLowerCase();
  return /^#[0-9a-f]{6}$/.test(text) ? text : null;
}

export function buildIndex(subnets: readonly SubnetSummary[], locale: string): SubnetIndex {
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  const byId = new Map(subnets.map((subnet) => [subnet.id, subnet]));
  const children = new Map<string | null, SubnetSummary[]>();
  for (const subnet of subnets) {
    const parent = subnet.parentId !== null && byId.has(subnet.parentId) ? subnet.parentId : null;
    const list = children.get(parent);
    if (list) {
      list.push(subnet);
    } else {
      children.set(parent, [subnet]);
    }
  }
  for (const list of children.values()) {
    list.sort((a, b) => collator.compare(a.name, b.name) || a.id.localeCompare(b.id));
  }
  return { byId, children };
}

/** The subnets from the top level down to `id` (empty if unknown). */
export function pathTo(index: SubnetIndex, id: string): SubnetSummary[] {
  const path: SubnetSummary[] = [];
  const seen = new Set<string>();
  let current = index.byId.get(id);
  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    path.unshift(current);
    current = current.parentId === null ? undefined : index.byId.get(current.parentId);
  }
  return path;
}

export function pathLabel(index: SubnetIndex, id: string): string {
  return pathTo(index, id)
    .map((subnet) => subnet.name)
    .join(PATH_SEPARATOR);
}

/** `id` and all subnets below it. */
export function subtree(index: SubnetIndex, id: string): Set<string> {
  const result = new Set<string>([id]);
  const stack = [id];
  while (stack.length > 0) {
    const next = stack.pop();
    for (const child of index.children.get(next ?? null) ?? []) {
      if (!result.has(child.id)) {
        result.add(child.id);
        stack.push(child.id);
      }
    }
  }
  return result;
}

/** Depth of a subnet: 1 on the top level. */
export function depthOf(index: SubnetIndex, id: string): number {
  return pathTo(index, id).length;
}

/** Subnets in tree order (parents before children, siblings by name). */
export function inTreeOrder(index: SubnetIndex): SubnetSummary[] {
  const result: SubnetSummary[] = [];
  const visit = (parent: string | null) => {
    for (const subnet of index.children.get(parent) ?? []) {
      result.push(subnet);
      visit(subnet.id);
    }
  };
  visit(null);
  return result;
}
