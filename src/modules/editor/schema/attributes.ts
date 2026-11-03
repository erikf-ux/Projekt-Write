/** Typed access to attributes of JSON nodes and fields of Markdown tokens. */
import type { JSONContent } from '@tiptap/core';

/** A string attribute of a JSON node, or null if it is missing or empty. */
export function stringAttribute(node: JSONContent, name: string): string | null {
  const value: unknown = (node.attrs as Record<string, unknown> | undefined)?.[name];
  if (typeof value === 'number') {
    return String(value);
  }
  return typeof value === 'string' && value !== '' ? value : null;
}

/** A string field of a Markdown token, or null. */
export function tokenString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}
