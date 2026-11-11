/**
 * One open document in the writing surface (section 12).
 *
 * * `.md`: rich text with the Markdown schema, and a source view (`Ctrl+/`)
 *   in the minimal schema. If the round-trip probe finds something the rich
 *   view cannot hold, the document opens in the source view and the reason
 *   is shown (section 10).
 * * `.txt`: the minimal schema only.
 *
 * A session lives as long as its tab, so the undo history does too; both
 * views keep their own history, and switching the view is one undo step.
 * Saving asks the session for the text. In the rich view the loss check
 * runs first, block by block, for changed blocks only (ADR 0014).
 */
import { Editor, type JSONContent } from '@tiptap/core';
import FindAndReplace from '@tiptap/extension-find-and-replace';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import { parseMarkdown, serializeMarkdown } from './markdown/adapter';
import { excerpt, findLosses, type Loss } from './markdown/losses';
import type { ConversionSource } from './convert';
import { firstDifference, markdownSchema, probeMarkdown, type ProbeIssue } from './markdown/probe';
import { plainClipboardText, plainDocument, plainExtensions, plainText } from './plain/schema';
import { isDocumentFile } from '@/modules/documents/importFiles';
import { mediaReference } from '@/modules/media/mediaUrl';
import { handleRichPaste } from './paste';
import { markdownExtensions } from './schema/extensions';
import { plainStatistics, richStatistics, selectionStatistics, type TextStatistics } from './statistics';
import { useStatistics } from './statisticsStore';

/** Search options of the find bar (`find/findStore.ts`). */
export interface SearchRequest {
  term: string;
  replacement: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  regex: boolean;
}

export interface SearchResults {
  count: number;
  /** 0-based index of the current result, or null. */
  current: number | null;
}

/** Statistics follow the text after this pause (counting is per block). */
const STATISTICS_DELAY_MS = 150;

export type EditorView = 'rich' | 'source';

/** What saving would write, or why it must ask first. */
export type SaveRequest = { kind: 'text'; text: string } | { kind: 'blocked'; losses: Loss[] };

export interface SessionOptions {
  id: string;
  format: DocumentFormat;
  text: string;
  /** Accessible name of the writing surface. */
  label: string;
  editable: boolean;
  /** The document changed (not on a view switch that keeps the text). */
  onChange: () => void;
  /** The writing surface lost the focus. */
  onBlur: () => void;
  /** Ctrl+click on a link: web links open in the system browser, internal ones in the app. */
  onOpenLink?: (href: string) => void;
  /** `[[` was typed in rich text: ask which document to link (ADR 0015). */
  onInternalLinkRequest?: () => void;
  /**
   * Files other than text documents were pasted or dropped into rich text:
   * they are imported into the library and inserted (ADR 0019, decision 10).
   * `position` is where they were dropped, null for the cursor.
   */
  onMediaFiles?: (files: File[], position: number | null) => void;
}

/**
 * A medium to insert: an image, or a link to a medium with its text (a PDF
 * reference with its page, or an attachment).
 */
export type InsertedMedium =
  | { kind: 'image'; id: string; alt: string }
  | { kind: 'link'; id: string; text: string; page: number | null };

export interface ViewSwitch {
  ok: boolean;
  /** Why the rich view is not possible (empty if `ok`). */
  issues: ProbeIssue[];
}

const SURFACE_CLASS = 'pw-surface';

/** An empty `.pwrite` document (one empty paragraph). */
const EMPTY_PWRITE: JSONContent = { type: 'doc', content: [{ type: 'paragraph' }] };

/**
 * The content of a `.pwrite` document as Rust hands it over (JSON of the
 * editor document), or null if it does not fit the schema.
 */
export function pwriteContent(text: string): JSONContent | null {
  try {
    const content = JSON.parse(text) as JSONContent;
    markdownSchema().nodeFromJSON(content).check();
    return content;
  } catch {
    return null;
  }
}

export class EditorSession {
  readonly id: string;
  readonly format: DocumentFormat;
  /** Holds both views; the panel of the tab shows it. */
  readonly root: HTMLDivElement;
  view: EditorView;
  /** Why the document opened in the source view instead of rich text. */
  readonly openIssues: ProbeIssue[];
  private readonly options: SessionOptions;
  private rich: Editor | null = null;
  private plain: Editor | null = null;
  private frontMatter = '';
  /** Loss kinds the user saved anyway ("Trotzdem speichern"). */
  private readonly accepted = new Set<Loss['kind']>();
  /** Loss check of top-level blocks; blocks keep their identity until changed. */
  private readonly checked = new WeakMap<ProseMirrorNode, Loss[]>();
  private silent = false;
  /** Counts of blocks and lines; nodes keep their identity until they change. */
  private readonly counted = new WeakMap<ProseMirrorNode, TextStatistics>();
  private statisticsTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(options: SessionOptions) {
    this.options = options;
    this.id = options.id;
    this.format = options.format;
    this.root = document.createElement('div');
    this.root.className = 'pw-session h-full';
    if (options.format === 'md') {
      const probe = probeMarkdown(options.text);
      this.openIssues = probe.issues;
      if (probe.issues.length === 0) {
        this.frontMatter = probe.document.frontMatter;
        this.view = 'rich';
        this.rich = this.createRich(probe.document.body);
      } else {
        this.view = 'source';
        this.plain = this.createPlain(options.text);
      }
    } else if (options.format === 'pwrite') {
      // `.pwrite` holds what the editor holds: no probe, no source view.
      const content = pwriteContent(options.text);
      this.openIssues = content ? [] : [{ kind: 'invalid' }];
      this.view = 'rich';
      this.rich = this.createRich(content ?? EMPTY_PWRITE);
    } else {
      this.openIssues = [];
      this.view = 'source';
      this.plain = this.createPlain(options.text);
    }
    // Counted after the text is shown: a long document opens sooner
    // (section 6: 50,000 words in ≤ 500 ms).
    this.scheduleStatistics();
  }

  /** Writes the statistics of the current view and selection to the store. */
  updateStatistics(): void {
    if (this.statisticsTimer !== null) {
      clearTimeout(this.statisticsTimer);
      this.statisticsTimer = null;
    }
    const editor = this.editor;
    const plain = editor === this.plain;
    const { doc, selection } = editor.state;
    const document = plain ? plainStatistics(doc, this.counted) : richStatistics(doc, this.counted);
    const selected = selection.empty ? null : selectionStatistics(doc, selection.from, selection.to, plain);
    useStatistics.getState().set(this.id, { document, selection: selected });
  }

  private scheduleStatistics(): void {
    if (this.statisticsTimer !== null) {
      clearTimeout(this.statisticsTimer);
    }
    this.statisticsTimer = setTimeout(() => {
      this.statisticsTimer = null;
      this.updateStatistics();
    }, STATISTICS_DELAY_MS);
  }

  get isMarkdown(): boolean {
    return this.format === 'md';
  }

  /** The editor of the current view. */
  get editor(): Editor {
    const editor = this.view === 'rich' ? this.rich : this.plain;
    if (!editor) {
      throw new Error('editor session without editor');
    }
    return editor;
  }

  /** The selection of the current view (stage 1 keeps it across a restart). */
  selection(): { anchor: number; head: number } {
    const { anchor, head } = this.editor.state.selection;
    return { anchor, head };
  }

  /** Puts the selection back where it was; positions beyond the end go to the end. */
  restoreSelection(anchor: number, head: number): void {
    const editor = this.editor;
    const end = editor.state.doc.content.size;
    const clamp = (position: number) => Math.max(1, Math.min(position, Math.max(1, end - 1)));
    editor
      .chain()
      .setTextSelection({ from: clamp(anchor), to: clamp(head) })
      .scrollIntoView()
      .run();
  }

  /** The text of the current view, as it would be saved (`.pwrite`: the content as JSON). */
  text(): string {
    if (this.format === 'pwrite') {
      return this.rich ? JSON.stringify(this.rich.getJSON()) : '';
    }
    if (this.view === 'rich' && this.rich) {
      return serializeMarkdown({ frontMatter: this.frontMatter, body: this.rich.getJSON() });
    }
    return this.plain ? plainText(this.plain.state.doc) : '';
  }

  /**
   * Inserts a link to a document with its title as text, in place of the
   * `[[` before the cursor (rich text only).
   */
  insertInternalLink(id: string, title: string): void {
    const editor = this.rich;
    if (!editor || (this.isMarkdown && this.view !== 'rich')) {
      return;
    }
    const { from } = editor.state.selection;
    const start = from >= 2 && editor.state.doc.textBetween(from - 2, from) === '[[' ? from - 2 : from;
    editor
      .chain()
      .focus()
      .deleteRange({ from: start, to: from })
      .insertContent({
        type: 'text',
        text: title,
        marks: [{ type: 'link', attrs: { href: `pwrite://${id}` } }],
      })
      .unsetMark('link')
      .run();
  }

  /**
   * Inserts media at the cursor or at `position`: images as image nodes,
   * PDF references and attachments as links. A single link over selected
   * text links that text. False if rich text is not shown or not editable.
   * The position comes from before an import and is kept inside the text.
   */
  insertMedia(items: readonly InsertedMedium[], position: number | null = null): boolean {
    const editor = this.rich;
    if (!editor || !editor.isEditable || (this.isMarkdown && this.view !== 'rich') || items.length === 0) {
      return false;
    }
    const [only] = items;
    if (items.length === 1 && only?.kind === 'link' && position === null && !editor.state.selection.empty) {
      return editor
        .chain()
        .focus()
        .setLink({ href: mediaReference(only.id, only.page) })
        .run();
    }
    const content: JSONContent[] = [];
    items.forEach((item, index) => {
      if (index > 0) {
        content.push({ type: 'text', text: ' ' });
      }
      content.push(
        item.kind === 'image'
          ? { type: 'image', attrs: { src: mediaReference(item.id), alt: item.alt, title: null } }
          : {
              type: 'text',
              text: item.text,
              marks: [{ type: 'link', attrs: { href: mediaReference(item.id, item.page) } }],
            },
      );
    });
    const chain = editor.chain().focus();
    const at = position === null ? null : Math.max(0, Math.min(position, editor.state.doc.content.size));
    (at === null ? chain.insertContent(content) : chain.insertContentAt(at, content)).unsetMark('link').run();
    return true;
  }

  /** What a conversion starts from: the rich text if it is shown, else the text. */
  conversionSource(): ConversionSource {
    if (this.rich && (this.format === 'pwrite' || (this.isMarkdown && this.view === 'rich'))) {
      return { kind: 'rich', content: this.rich.getJSON(), frontMatter: this.frontMatter };
    }
    return { kind: 'text', text: this.text() };
  }

  /** The text to save, or the losses the user has not accepted yet. */
  saveRequest(): SaveRequest {
    if (this.isMarkdown && this.view === 'rich' && this.rich) {
      const losses = this.losses().filter((loss) => !this.accepted.has(loss.kind));
      if (losses.length > 0) {
        return { kind: 'blocked', losses };
      }
    }
    return { kind: 'text', text: this.text() };
  }

  /** "Trotzdem speichern": these kinds no longer stop saving in this tab. */
  acceptLosses(losses: readonly Loss[]): void {
    for (const loss of losses) {
      this.accepted.add(loss.kind);
    }
  }

  /** Everything Markdown would lose, block by block (section 10). */
  losses(): Loss[] {
    const losses: Loss[] = [];
    if (!this.rich || !this.isMarkdown) {
      return losses;
    }
    this.rich.state.doc.forEach((block, _offset, index) => {
      let found = this.checked.get(block);
      if (!found) {
        found = checkBlock(block.toJSON() as JSONContent);
        this.checked.set(block, found);
      }
      losses.push(...found.map((loss) => ({ ...loss, block: index })));
    });
    return losses;
  }

  /**
   * Switches between rich text and source view (Markdown only). The text
   * goes over unchanged; to rich text only if the probe finds nothing.
   */
  toggleView(): ViewSwitch {
    if (!this.isMarkdown) {
      return { ok: false, issues: [] };
    }
    if (this.view === 'rich') {
      const text = this.text();
      if (this.plain) {
        this.replace(this.plain, plainDocument(text));
      } else {
        this.plain = this.createPlain(text);
      }
      this.show('source');
      return { ok: true, issues: [] };
    }
    const probe = probeMarkdown(this.text());
    if (probe.issues.length > 0) {
      return { ok: false, issues: probe.issues };
    }
    this.frontMatter = probe.document.frontMatter;
    if (this.rich) {
      this.replace(this.rich, probe.document.body);
      this.trust(this.rich);
    } else {
      this.rich = this.createRich(probe.document.body);
    }
    this.show('rich');
    return { ok: true, issues: [] };
  }

  /** Runs a search in the current view (highlights all results). */
  search(request: SearchRequest): void {
    const commands = this.editor.commands;
    commands.setCaseSensitive(request.caseSensitive);
    commands.setWholeWord(request.wholeWord);
    commands.setUseRegex(request.regex);
    commands.setReplaceTerm(request.replacement);
    commands.setSearchTerm(request.term);
  }

  clearSearch(): void {
    for (const editor of [this.rich, this.plain]) {
      editor?.commands.clearSearch();
    }
  }

  searchResults(): SearchResults {
    const storage = this.editor.storage.findAndReplace;
    return { count: storage.results.length, current: storage.currentIndex };
  }

  findNext(): void {
    this.editor.commands.goToNextResult();
  }

  /** Selects the first result and scrolls it into view (global search, Phase 9). */
  revealFirstResult(): void {
    if (this.searchResults().count === 0) {
      return;
    }
    // From result 0, "next" and back is result 0, selected and scrolled to.
    this.editor.commands.goToNextResult();
    this.editor.commands.goToPreviousResult();
  }

  findPrevious(): void {
    this.editor.commands.goToPreviousResult();
  }

  replaceCurrent(): void {
    this.editor.commands.replace();
  }

  replaceAll(): void {
    this.editor.commands.replaceAll();
  }

  setEditable(editable: boolean): void {
    this.options.editable = editable;
    this.rich?.setEditable(editable, false);
    this.plain?.setEditable(editable, false);
  }

  focus(): void {
    this.editor.commands.focus();
  }

  /** The accessible name follows the title of the document. */
  setLabel(label: string): void {
    this.options.label = label;
    for (const editor of [this.rich, this.plain]) {
      editor?.view.dom.setAttribute('aria-label', label);
    }
  }

  destroy(): void {
    if (this.statisticsTimer !== null) {
      clearTimeout(this.statisticsTimer);
    }
    useStatistics.getState().remove(this.id);
    this.rich?.destroy();
    this.plain?.destroy();
    this.root.remove();
  }

  private show(view: EditorView): void {
    this.view = view;
    for (const editor of [this.rich, this.plain]) {
      const host = editor?.view.dom.parentElement;
      if (host) {
        host.hidden = editor !== this.editor;
      }
    }
    this.updateStatistics();
  }

  /** New content as one undo step, without counting as a change. */
  private replace(editor: Editor, content: JSONContent): void {
    this.silent = true;
    try {
      editor.commands.setContent(content, { emitUpdate: true });
    } finally {
      this.silent = false;
    }
  }

  private host(): HTMLDivElement {
    const host = document.createElement('div');
    host.className = 'h-full';
    this.root.append(host);
    return host;
  }

  /** A rich editor for content the probe has checked: its blocks need no check. */
  private createRich(content: JSONContent): Editor {
    const editor = this.createEditor([...markdownExtensions(), findExtension()], content, 'pw-rich');
    this.trust(editor);
    return editor;
  }

  private trust(editor: Editor): void {
    editor.state.doc.forEach((block) => {
      this.checked.set(block, []);
    });
  }

  private createPlain(text: string): Editor {
    return this.createEditor(
      [...plainExtensions(), findExtension()],
      plainDocument(text),
      this.isMarkdown ? 'pw-source' : 'pw-plain',
    );
  }

  private createEditor(
    extensions: Editor['options']['extensions'],
    content: JSONContent,
    variant: string,
  ): Editor {
    const isPlain = variant !== 'pw-rich';
    return new Editor({
      element: this.host(),
      extensions,
      content,
      editable: this.options.editable,
      injectCSS: false,
      enableInputRules: !isPlain,
      enablePasteRules: !isPlain,
      editorProps: {
        attributes: {
          'data-testid': 'editor',
          'data-view': variant.slice(3),
          role: 'textbox',
          'aria-multiline': 'true',
          'aria-label': this.options.label,
          spellcheck: 'false',
          class: `${SURFACE_CLASS} ${variant}`,
        },
        ...(isPlain
          ? {
              clipboardTextSerializer: plainClipboardText,
              // Pasted HTML becomes its text: the minimal schema has no formatting.
              handlePaste: (view, event) => {
                const text = event.clipboardData?.getData('text/plain');
                if (text === undefined) {
                  return false;
                }
                view.pasteText(text.replace(/\r\n?/g, '\n'));
                return true;
              },
            }
          : {
              // The second `[` of `[[` is typed, then the picker asks for the document.
              handleTextInput: (view, from, _to, text) => {
                const { $from } = view.state.selection;
                const inCode =
                  $from.parent.type.spec.code === true ||
                  $from.marks().some((mark) => mark.type.name === 'code');
                if (
                  text === '[' &&
                  from > 0 &&
                  !inCode &&
                  view.state.doc.textBetween(from - 1, from) === '['
                ) {
                  setTimeout(() => {
                    this.options.onInternalLinkRequest?.();
                  }, 0);
                }
                return false;
              },
              // Ctrl+click opens a link; a plain click only places the cursor.
              handleClick: (view, position, event) => {
                if (!(event.ctrlKey || event.metaKey)) {
                  return false;
                }
                const href = linkAt(view.state.doc, position);
                if (href === null) {
                  return false;
                }
                this.options.onOpenLink?.(href);
                return true;
              },
              // The webview never follows a link in the text itself.
              handleDOMEvents: {
                click: (_view, event) => {
                  if (event.target instanceof Element && event.target.closest('a') !== null) {
                    event.preventDefault();
                  }
                  return false;
                },
                auxclick: (_view, event) => {
                  if (event.target instanceof Element && event.target.closest('a') !== null) {
                    event.preventDefault();
                  }
                  return false;
                },
              },
              handlePaste: (view, event) => {
                const files = mediaFiles(event.clipboardData);
                if (files.length > 0 && this.options.onMediaFiles) {
                  this.options.onMediaFiles(files, null);
                  return true;
                }
                return handleRichPaste(view, event, (content) => {
                  this.rich?.commands.insertContent(content);
                });
              },
              // Dropped media files land where they were dropped; text
              // documents are left to the window (document import).
              handleDrop: (view, event, _slice, moved) => {
                const files = moved ? [] : mediaFiles(event.dataTransfer);
                if (files.length === 0 || !this.options.onMediaFiles || !view.editable) {
                  return false;
                }
                const position = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos ?? null;
                this.options.onMediaFiles(files, position);
                return true;
              },
            }),
      },
      onUpdate: () => {
        if (!this.silent) {
          this.options.onChange();
        }
      },
      onTransaction: ({ transaction }) => {
        if (transaction.docChanged || transaction.selectionSet) {
          this.scheduleStatistics();
        }
      },
      onBlur: () => {
        this.options.onBlur();
      },
    });
  }
}

/** The address of the link at a position, if there is one. */
/** Files of a paste or drop that are no text documents. */
function mediaFiles(data: DataTransfer | null): File[] {
  // Not every clipboard (or test double) carries a file list.
  const files: FileList | undefined = data?.files;
  return files ? [...files].filter((file) => !isDocumentFile(file.name)) : [];
}

export function linkAt(document: ProseMirrorNode, position: number): string | null {
  for (const at of [position, position - 1]) {
    if (at < 0 || at >= document.content.size) {
      continue;
    }
    const link = document.nodeAt(at)?.marks.find((mark) => mark.type.name === 'link');
    const href: unknown = link?.attrs['href'];
    if (typeof href === 'string' && href !== '') {
      return href;
    }
  }
  return null;
}

/** Find and replace (section 12); highlights styled in `editor.css` (no inline style, CSP). */
function findExtension() {
  return FindAndReplace.configure({ injectCSS: false, searchDebounceMs: 0 });
}

/**
 * Losses of one top-level block. The round trip decides; the named rules
 * of `findLosses` only explain a block that comes back different.
 */
export function checkBlock(block: JSONContent): Loss[] {
  const document: JSONContent = { type: 'doc', content: [block] };
  const again = markdownSchema()
    .nodeFromJSON(parseMarkdown(serializeMarkdown({ frontMatter: '', body: document })).body)
    .toJSON() as JSONContent;
  if (firstDifference(document, again) === null) {
    return [];
  }
  const named = findLosses(document);
  return named.length > 0 ? named : [{ kind: 'changed', block: 0, excerpt: excerpt(block) }];
}

const sessions = new Map<string, EditorSession>();

export function getSession(id: string): EditorSession | undefined {
  return sessions.get(id);
}

/** The session of an open document, created on first use. */
export function openSession(options: SessionOptions): EditorSession {
  const existing = sessions.get(options.id);
  if (existing) {
    return existing;
  }
  const session = new EditorSession(options);
  sessions.set(options.id, session);
  return session;
}

export function closeSession(id: string): void {
  sessions.get(id)?.destroy();
  sessions.delete(id);
}

export function closeAllSessions(): void {
  for (const id of [...sessions.keys()]) {
    closeSession(id);
  }
}

export function setSessionsEditable(editable: boolean): void {
  for (const session of sessions.values()) {
    session.setEditable(editable);
  }
}
