/**
 * For tests: the marked instance of the Markdown manager (with the quick
 * checks of `modules/editor/markdown/manager.ts`) and marked as it is, with
 * the same extensions. Both must read every text into the same tokens.
 */
import { Marked } from 'marked';
import { markdownManager } from '@/modules/editor/markdown/adapter';

let plain: Marked | null = null;

function instances(): { checked: Marked; plain: Marked } {
  const checked = markdownManager().instance as unknown as Marked;
  if (!plain) {
    plain = new Marked();
    plain.defaults = { ...plain.defaults, extensions: checked.defaults.extensions ?? null };
  }
  return { checked, plain };
}

/** The tokens (and link definitions) of `text`, read by both instances. */
export function lexBoth(text: string): { checked: string; plain: string } {
  const both = instances();
  const lex = (instance: Marked) => {
    const tokens = new instance.Lexer(instance.defaults).lex(text);
    return JSON.stringify({ tokens, links: tokens.links });
  };
  return { checked: lex(both.checked), plain: lex(both.plain) };
}
