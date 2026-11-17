/**
 * The language check in the editor (section 14, ADR 0025).
 *
 * - **Checking:** after a pause in typing, never in the input event, the
 *   paragraphs without a cached result go to Rust in batches, the ones near
 *   the cursor first. Results are cached by document, language and text;
 *   unchanged paragraphs keep their node and need no work.
 * - **Underlines:** spelling and context red and wavy, grammar and
 *   punctuation blue and dashed (`editor.css`). An edited paragraph loses
 *   its underlines until it is checked again.
 * - **Autocorrect:** when a word is finished (space, punctuation, Enter),
 *   never during a composition, Rust decides with the uniqueness rule. The
 *   answer comes later and is dropped when the word has changed. A
 *   correction is its own undo step and briefly marked; `Ctrl + Z` brings
 *   the typed word back and records the correction as rejected.
 */
import { Extension, getChangedRanges } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { closeHistory } from '@tiptap/pm/history';
import { Plugin, PluginKey, type EditorState, type Transaction } from '@tiptap/pm/state';
import { Decoration, DecorationSet, type EditorView } from '@tiptap/pm/view';
import { call } from '@/services/ipc';
import type { CheckedText } from '@/types/generated/CheckedText';
import type { Finding } from '@/types/generated/Finding';
import type { Span } from '@/types/generated/Span';
import { blockText, wordBefore, type CheckPart } from './extract';
import { checkSettings, documentLanguage, useLanguage } from './languageStore';

/** Pause after typing before paragraphs are checked. */
export const CHECK_DELAY_MS = 400;
/** How long a correction stays marked. */
export const FLASH_MS = 1500;
const BATCH_TEXTS = 60;
const BATCH_CHARS = 30_000;
const CACHE_LIMIT = 40_000;
/** Characters that finish a word. */
const WORD_END = /^[\s.,;:!?)\]}"'»«“”‘’…]$/u;

export interface FindingSpec {
  finding: Finding;
  /** The language the text was checked in. */
  language: string;
  /** Where in the document the finding starts and ends. */
  from: number;
  to: number;
}

interface Pending {
  id: number;
  from: number;
  to: number;
  word: string;
}

interface Correction {
  from: number;
  to: number;
  original: string;
  correction: string;
}

interface LanguageState {
  decorations: DecorationSet;
  pending: Pending[];
  last: Correction | null;
  flash: { from: number; to: number } | null;
  /** A correction Ctrl+Z took back, to be recorded. */
  reject: { word: string; correction: string } | null;
}

type Meta =
  | { kind: 'refresh'; decorations: DecorationSet }
  | { kind: 'pending'; pending: Pending }
  | { kind: 'drop'; id: number }
  | { kind: 'corrected'; id: number; correction: Correction }
  | { kind: 'unflash' }
  | { kind: 'rejected' };

export const languageCheckKey = new PluginKey<LanguageState>('languageCheck');

/** Results by document, language and text. */
const cache = new Map<string, CheckedText>();

function remember(key: string, value: CheckedText): void {
  if (cache.size >= CACHE_LIMIT) {
    // The oldest tenth goes (a Map keeps the order of insertion).
    let drop = Math.ceil(CACHE_LIMIT / 10);
    for (const old of cache.keys()) {
      cache.delete(old);
      drop -= 1;
      if (drop === 0) {
        break;
      }
    }
  }
  cache.set(key, value);
}

/** Forgets every cached result (tests). */
export function clearCheckCache(): void {
  cache.clear();
}

function cacheKey(documentKey: string, epoch: number, part: CheckPart): string {
  return `${documentKey}\u0001${String(epoch)}\u0001${part.language ?? ''}\u0001${part.text}`;
}

const KIND_CLASS: Record<Finding['kind'], string> = {
  spelling: 'pw-check pw-check-spelling',
  context: 'pw-check pw-check-context',
  grammar: 'pw-check pw-check-grammar',
  punctuation: 'pw-check pw-check-punctuation',
};

/** The decorations of every checked paragraph whose result is cached. */
function buildDecorations(doc: ProseMirrorNode, documentKey: string, epoch: number): DecorationSet {
  const ignored = useLanguage.getState().ignored;
  const decorations: Decoration[] = [];
  doc.descendants((node, pos) => {
    if (!node.isTextblock) {
      return true;
    }
    const block = blockText(node);
    if (!block) {
      return false;
    }
    for (const part of block.parts) {
      const checked = cache.get(cacheKey(documentKey, epoch, part));
      if (!checked) {
        continue;
      }
      const base = pos + 1 + part.offset;
      for (const finding of checked.findings) {
        const from = base + finding.start;
        const to = base + finding.end;
        if (to <= from || to > pos + node.nodeSize - 1) {
          continue;
        }
        if (finding.kind === 'spelling' && ignored.has(doc.textBetween(from, to))) {
          continue;
        }
        const spec: FindingSpec = { finding, language: checked.language, from, to };
        decorations.push(
          Decoration.inline(
            from,
            to,
            { class: KIND_CLASS[finding.kind], 'data-check-rule': finding.rule },
            { ...spec, inclusiveStart: false, inclusiveEnd: false },
          ),
        );
      }
    }
    return false;
  });
  return DecorationSet.create(doc, decorations);
}

/**
 * Maps a range so that it grows with text typed at its edges: the text
 * comparison afterwards then tells whether the word is still the same.
 */
function mapRange<T extends { from: number; to: number }>(range: T, tr: Transaction): T | null {
  const from = tr.mapping.map(range.from, -1);
  const to = tr.mapping.map(range.to, 1);
  return to < from ? null : { ...range, from, to };
}

function applyState(tr: Transaction, value: LanguageState, newState: EditorState): LanguageState {
  const meta = tr.getMeta(languageCheckKey) as Meta | undefined;
  let next = value;
  if (tr.docChanged) {
    let decorations = value.decorations.map(tr.mapping, tr.doc);
    // An edited paragraph loses its underlines until it is checked again.
    for (const { newRange } of getChangedRanges(tr)) {
      const from = Math.max(0, Math.min(newRange.from, tr.doc.content.size));
      const to = Math.max(from, Math.min(newRange.to, tr.doc.content.size));
      const $from = tr.doc.resolve(from);
      const $to = tr.doc.resolve(to);
      const start = $from.parent.isTextblock ? $from.start() : from;
      const end = $to.parent.isTextblock ? $to.end() : to;
      decorations = decorations.remove(decorations.find(start, end));
    }
    const pending = value.pending.flatMap((p) => mapRange(p, tr) ?? []);
    let last = value.last ? mapRange(value.last, tr) : null;
    let reject = value.reject;
    // Ctrl+Z on a correction: the typed word is back (prosemirror-history
    // marks its undo transactions with this key).
    const history = tr.getMeta('history$') as { redo: boolean } | undefined;
    if (history && !history.redo && last) {
      const end = last.from + last.original.length;
      if (end <= newState.doc.content.size && newState.doc.textBetween(last.from, end) === last.original) {
        reject = { word: last.original, correction: last.correction };
        last = null;
      }
    }
    const flash = value.flash ? mapRange(value.flash, tr) : null;
    next = { decorations, pending, last, flash, reject };
  }
  if (!meta) {
    return next;
  }
  switch (meta.kind) {
    case 'refresh':
      return { ...next, decorations: meta.decorations };
    case 'pending':
      return { ...next, pending: [...next.pending, meta.pending] };
    case 'drop':
      return { ...next, pending: next.pending.filter((p) => p.id !== meta.id) };
    case 'corrected':
      return {
        ...next,
        pending: next.pending.filter((p) => p.id !== meta.id),
        last: meta.correction,
        flash: { from: meta.correction.from, to: meta.correction.to },
      };
    case 'unflash':
      return { ...next, flash: null };
    case 'rejected':
      return { ...next, reject: null };
  }
}

/** The finding under a position, if any. */
export function findingAt(state: EditorState, position: number): FindingSpec | null {
  const value = languageCheckKey.getState(state);
  if (!value) {
    return null;
  }
  const found = value.decorations.find(position, position, (spec) => 'finding' in spec);
  const decoration = found.find((d) => d.from <= position && position <= d.to) ?? found[0];
  if (!decoration) {
    return null;
  }
  const spec = decoration.spec as FindingSpec;
  return { ...spec, from: decoration.from, to: decoration.to };
}

/** Replaces a finding with a replacement, as one undo step. */
export function replaceFinding(view: EditorView, finding: FindingSpec, replacement: string): void {
  const { state } = view;
  if (finding.to > state.doc.content.size) {
    return;
  }
  const tr = state.tr.insertText(replacement, finding.from, finding.to);
  view.dispatch(closeHistory(tr));
}

export interface LanguageCheckOptions {
  /** The library document; null for an external file. */
  documentId: string | null;
  /** False for the Markdown source view: its syntax is not text. */
  enabled: boolean;
}

/** Where a finished word stands (condition 6). */
function spanOf(
  state: EditorState,
  from: number,
  part: CheckPart | undefined,
  documentId: string | null,
): Span {
  const $from = state.doc.resolve(from);
  for (let depth = $from.depth; depth > 0; depth -= 1) {
    if ($from.node(depth).type.name === 'blockquote') {
      return 'quote';
    }
  }
  if (part?.language && part.language !== documentLanguage(documentId)) {
    return 'otherLanguage';
  }
  return 'text';
}

class Checker {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private flashTimer: ReturnType<typeof setTimeout> | null = null;
  private running = false;
  private destroyed = false;
  private nextId = 1;
  private readonly unsubscribe: () => void;

  constructor(
    private view: EditorView,
    private readonly options: LanguageCheckOptions,
  ) {
    this.unsubscribe = useLanguage.subscribe((state, previous) => {
      if (state.epoch !== previous.epoch) {
        this.schedule(0);
      }
    });
    if (options.documentId) {
      void useLanguage
        .getState()
        .loadProfile(options.documentId)
        .then(() => {
          this.schedule(0);
        });
    } else {
      this.schedule(0);
    }
  }

  private get documentKey(): string {
    return this.options.documentId ?? 'external';
  }

  update(view: EditorView, previous: EditorState): void {
    this.view = view;
    if (view.state.doc !== previous.doc) {
      this.schedule();
    }
    const value = languageCheckKey.getState(view.state);
    if (value?.reject) {
      const { word, correction } = value.reject;
      void call('language_reject', { word, correction });
      this.dispatchMeta({ kind: 'rejected' });
    }
  }

  /** Read through a method: the check runs across awaits. */
  private isDestroyed(): boolean {
    return this.destroyed;
  }

  destroy(): void {
    this.destroyed = true;
    this.unsubscribe();
    if (this.timer !== null) {
      clearTimeout(this.timer);
    }
    if (this.flashTimer !== null) {
      clearTimeout(this.flashTimer);
    }
  }

  private dispatchMeta(meta: Meta): void {
    if (this.destroyed) {
      return;
    }
    // Never in the middle of a composition: it would break it.
    if (this.view.composing) {
      setTimeout(() => {
        this.dispatchMeta(meta);
      }, 50);
      return;
    }
    this.view.dispatch(this.view.state.tr.setMeta(languageCheckKey, meta));
  }

  schedule(delay = CHECK_DELAY_MS): void {
    if (this.destroyed) {
      return;
    }
    if (this.timer !== null) {
      clearTimeout(this.timer);
    }
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.run();
    }, delay);
  }

  private refresh(): void {
    const epoch = useLanguage.getState().epoch;
    this.dispatchMeta({
      kind: 'refresh',
      decorations: buildDecorations(this.view.state.doc, this.documentKey, epoch),
    });
  }

  private async run(): Promise<void> {
    if (this.destroyed || this.running) {
      return;
    }
    const settings = checkSettings();
    if (!this.options.enabled || !settings.enabled) {
      const value = languageCheckKey.getState(this.view.state);
      if (value && value.decorations !== DecorationSet.empty) {
        this.dispatchMeta({ kind: 'refresh', decorations: DecorationSet.empty });
      }
      return;
    }
    if (this.view.composing) {
      this.schedule();
      return;
    }
    const epoch = useLanguage.getState().epoch;
    const { doc, selection } = this.view.state;
    const missing: { key: string; part: CheckPart; position: number }[] = [];
    const seen = new Set<string>();
    doc.descendants((node, pos) => {
      if (!node.isTextblock) {
        return true;
      }
      const block = blockText(node);
      for (const part of block?.parts ?? []) {
        const key = cacheKey(this.documentKey, epoch, part);
        if (!cache.has(key) && !seen.has(key)) {
          seen.add(key);
          missing.push({ key, part, position: pos });
        }
      }
      return false;
    });
    if (missing.length === 0) {
      this.refresh();
      return;
    }
    missing.sort((a, b) => Math.abs(a.position - selection.head) - Math.abs(b.position - selection.head));
    const batch: typeof missing = [];
    let chars = 0;
    for (const item of missing) {
      if (batch.length >= BATCH_TEXTS || (batch.length > 0 && chars + item.part.text.length > BATCH_CHARS)) {
        break;
      }
      batch.push(item);
      chars += item.part.text.length;
    }
    this.running = true;
    try {
      const result = await call('language_check', {
        ...(this.options.documentId ? { documentId: this.options.documentId } : {}),
        texts: batch.map((item) =>
          item.part.language
            ? { text: item.part.text, language: item.part.language }
            : { text: item.part.text },
        ),
      });
      // An answer of another shape (a test double) counts as none.
      const values: unknown = result.ok ? result.value : null;
      if (!Array.isArray(values) || this.isDestroyed()) {
        return;
      }
      (values as CheckedText[]).forEach((checked, index) => {
        const item = batch[index];
        if (item) {
          remember(item.key, checked);
        }
      });
    } finally {
      this.running = false;
    }
    this.refresh();
    if (missing.length > batch.length) {
      this.schedule(0);
    }
  }

  /** A word was finished at `end` (before the typed character). */
  wordFinished(state: EditorState, end: number): void {
    const settings = checkSettings();
    if (!this.options.enabled || !settings.enabled || !settings.autocorrect) {
      return;
    }
    const profile = this.options.documentId
      ? useLanguage.getState().profiles[this.options.documentId]
      : undefined;
    if (profile && !profile.autocorrect) {
      return;
    }
    const $end = state.doc.resolve(end);
    if (!$end.parent.isTextblock) {
      return;
    }
    const block = blockText($end.parent);
    if (!block) {
      return;
    }
    const offset = end - $end.start();
    const found = wordBefore(block.text, offset);
    if (!found || found.word.length < 4) {
      return;
    }
    const part = block.parts.find((p) => p.offset <= found.start && found.start < p.offset + p.text.length);
    const pending: Pending = {
      id: this.nextId,
      from: $end.start() + found.start,
      to: end,
      word: found.word,
    };
    this.nextId += 1;
    const span = spanOf(state, pending.from, part, this.options.documentId);
    queueMicrotask(() => {
      this.dispatchMeta({ kind: 'pending', pending });
      void this.ask(pending, span, part?.language ?? null);
    });
  }

  private async ask(pending: Pending, span: Span, language: string | null): Promise<void> {
    const result = await call('language_autocorrect', {
      ...(this.options.documentId ? { documentId: this.options.documentId } : {}),
      word: pending.word,
      ...(language ? { language } : {}),
      span,
    });
    if (this.isDestroyed()) {
      return;
    }
    const value = languageCheckKey.getState(this.view.state);
    const current = value?.pending.find((p) => p.id === pending.id);
    const answer: unknown = result.ok ? result.value : null;
    const correction =
      typeof answer === 'object' &&
      answer !== null &&
      'correction' in answer &&
      typeof answer.correction === 'string'
        ? answer.correction
        : null;
    const { doc } = this.view.state;
    if (
      !current ||
      !correction ||
      this.view.composing ||
      current.to > doc.content.size ||
      doc.textBetween(current.from, current.to) !== current.word
    ) {
      this.dispatchMeta({ kind: 'drop', id: pending.id });
      return;
    }
    const tr = this.view.state.tr.insertText(correction, current.from, current.to);
    tr.setMeta(languageCheckKey, {
      kind: 'corrected',
      id: pending.id,
      correction: {
        from: current.from,
        to: current.from + correction.length,
        original: current.word,
        correction,
      },
    } satisfies Meta);
    this.view.dispatch(closeHistory(tr));
    if (this.flashTimer !== null) {
      clearTimeout(this.flashTimer);
    }
    this.flashTimer = setTimeout(() => {
      this.flashTimer = null;
      this.dispatchMeta({ kind: 'unflash' });
    }, FLASH_MS);
  }
}

const checkers = new WeakMap<EditorView, Checker>();

export const LanguageCheck = Extension.create<LanguageCheckOptions>({
  name: 'languageCheck',

  addOptions() {
    return { documentId: null, enabled: true };
  },

  addProseMirrorPlugins() {
    const options = this.options;
    return [
      new Plugin<LanguageState>({
        key: languageCheckKey,
        state: {
          init: () => ({
            decorations: DecorationSet.empty,
            pending: [],
            last: null,
            flash: null,
            reject: null,
          }),
          apply: (tr, value, _old, newState) => applyState(tr, value, newState),
        },
        view: (view) => {
          const checker = new Checker(view, options);
          checkers.set(view, checker);
          return {
            update: (next, previous) => {
              checker.update(next, previous);
            },
            destroy: () => {
              checkers.delete(view);
              checker.destroy();
            },
          };
        },
        props: {
          decorations: (state) => {
            const value = languageCheckKey.getState(state);
            if (!value) {
              return null;
            }
            if (!value.flash || value.flash.to > state.doc.content.size) {
              return value.decorations;
            }
            return value.decorations.add(state.doc, [
              Decoration.inline(value.flash.from, value.flash.to, { class: 'pw-check-corrected' }),
            ]);
          },
          handleTextInput: (view, from, _to, text) => {
            if (!view.composing && WORD_END.test(text)) {
              checkers.get(view)?.wordFinished(view.state, from);
            }
            return false;
          },
          handleKeyDown: (view, event) => {
            if (
              event.key === 'Enter' &&
              !event.isComposing &&
              !view.composing &&
              view.state.selection.empty
            ) {
              checkers.get(view)?.wordFinished(view.state, view.state.selection.from);
            }
            return false;
          },
        },
      }),
    ];
  },
});
