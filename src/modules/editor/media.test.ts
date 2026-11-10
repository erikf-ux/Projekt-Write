import { afterEach, describe, expect, it, vi } from 'vitest';
import { selectedImage } from './ImageDialog';
import { matchingMedia, pageOf } from './MediaPicker';
import { cssSize } from './schema/image';
import { closeAllSessions, openSession, type SessionOptions } from './session';
import type { MediaSummary } from '@/types/generated/MediaSummary';

const IMAGE = '0b6d3c9e-2f41-4a7e-9c55-1d2e3f405162';
const PDF = '7a1e2b3c-4d5e-4f60-8172-93a4b5c6d7e8';

function session(format: 'md' | 'pwrite', text: string, extra: Partial<SessionOptions> = {}) {
  const created = openSession({
    id: `doc-${format}-${String(Math.random())}`,
    format,
    text,
    label: 'Schreibfläche: Test',
    editable: true,
    onChange: () => undefined,
    onBlur: () => undefined,
    ...extra,
  });
  document.body.append(created.root);
  return created;
}

function medium(id: string, kind: MediaSummary['kind'], title: string, extra: Partial<MediaSummary> = {}) {
  return {
    id,
    kind,
    mimeType: kind === 'pdf' ? 'application/pdf' : 'image/png',
    title,
    description: '',
    originalName: `${title}.png`,
    importFolder: null,
    sizeBytes: 10,
    width: null,
    height: null,
    pageCount: null,
    createdAt: '',
    updatedAt: '',
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
    ...extra,
  } satisfies MediaSummary;
}

afterEach(() => {
  closeAllSessions();
});

describe('media in the editor', () => {
  it('shows a library image through the handler with its caption and size', () => {
    const text = `Vorher <img src="pwmedia://${IMAGE}" alt="Hafen" title="Der Hafen am Morgen" width="50%"> nachher\n`;
    const markdown = session('md', text);
    expect(markdown.view).toBe('rich');
    const figure = markdown.root.querySelector<HTMLElement>(`[data-pw-media-image="${IMAGE}"]`);
    expect(figure).not.toBeNull();
    expect(figure?.style.width).toBe('50%');
    const picture = figure?.querySelector('img');
    expect(picture?.getAttribute('src')).toMatch(
      new RegExp(`^(pwmedia://localhost/|http://pwmedia\\.localhost/)${IMAGE}$`),
    );
    expect(picture?.alt).toBe('Hafen');
    expect(figure?.querySelector('.pw-media-caption')?.textContent).toBe('Der Hafen am Morgen');
    // Shown, not changed: the text stays as it was.
    expect(markdown.text()).toBe(text);
  });

  it('shows "Medium fehlt" when the handler has no such medium', () => {
    const markdown = session('md', `![Weg](pwmedia://${IMAGE})\n`);
    const picture = markdown.root.querySelector(`[data-pw-media-image="${IMAGE}"] img`);
    picture?.dispatchEvent(new Event('error'));
    const missing = markdown.root.querySelector('[data-pw-media-missing]');
    expect(missing?.textContent).toMatch(/Medium fehlt|Medium missing/);
    expect(markdown.text()).toBe(`![Weg](pwmedia://${IMAGE})\n`);
  });

  it('selects a library image on a click', () => {
    const markdown = session('md', `Vorher ![Hafen](pwmedia://${IMAGE}) nachher\n`);
    const figure = markdown.root.querySelector(`[data-pw-media-image="${IMAGE}"]`);
    figure?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, button: 0 }));
    const { selection } = markdown.editor.state;
    expect(selection.from).toBe(8);
    expect(markdown.editor.state.doc.nodeAt(selection.from)?.type.name).toBe('image');
    expect(selection.to - selection.from).toBe(1);
  });

  it('edits the selected image or the one next to the cursor', () => {
    const markdown = session('md', `Vorher ![Hafen](pwmedia://${IMAGE}) nachher\n`);
    markdown.editor.commands.setNodeSelection(8);
    expect(selectedImage(markdown.id)).toBe(8);
    markdown.editor.commands.setTextSelection(8);
    expect(selectedImage(markdown.id)).toBe(8);
    markdown.editor.commands.setTextSelection(9);
    expect(selectedImage(markdown.id)).toBe(8);
    markdown.editor.commands.setTextSelection(3);
    expect(selectedImage(markdown.id)).toBeNull();
    markdown.editor.commands.setTextSelection({ from: 2, to: 9 });
    expect(selectedImage(markdown.id)).toBeNull();
  });

  it('keeps images with a web address as notes that load nothing', () => {
    const markdown = session('md', '![Logo](https://example.org/logo.png)\n');
    expect(markdown.root.querySelector('.ProseMirror img:not(.ProseMirror-separator)')).toBeNull();
    expect(markdown.root.querySelector('[data-pw-image]')?.textContent).toBe('Logo');
  });

  it('inserts images and PDF references as Markdown and .pwrite', () => {
    const markdown = session('md', 'Text\n');
    markdown.editor.commands.setTextSelection(5);
    expect(
      markdown.insertMedia([
        { kind: 'image', id: IMAGE, alt: 'Hafen' },
        { kind: 'link', id: PDF, text: 'Studie (Seite 3)', page: 3 },
      ]),
    ).toBe(true);
    expect(markdown.text()).toBe(
      `Text![Hafen](pwmedia://${IMAGE}) [Studie (Seite 3)](pwmedia://${PDF}#page=3)\n`,
    );

    const rich = session('pwrite', JSON.stringify({ type: 'doc', content: [{ type: 'paragraph' }] }));
    rich.insertMedia([{ kind: 'image', id: IMAGE, alt: 'Hafen' }]);
    expect(rich.text()).toContain(`"src":"pwmedia://${IMAGE}"`);
  });

  it('links selected text with a PDF reference', () => {
    const markdown = session('md', 'Siehe die Studie.\n');
    markdown.editor.commands.setTextSelection({ from: 11, to: 17 });
    markdown.insertMedia([{ kind: 'link', id: PDF, text: 'unused', page: 12 }]);
    expect(markdown.text()).toBe(`Siehe die [Studie](pwmedia://${PDF}#page=12).\n`);
  });

  it('hands pasted image files to the import instead of the clipboard text', () => {
    const onMediaFiles = vi.fn();
    const markdown = session('md', 'Text\n', { onMediaFiles });
    const image = new File(['png'], 'image.png', { type: 'image/png' });
    const note = new File(['# x'], 'Notiz.md', { type: 'text/markdown' });
    const event = new Event('paste', { bubbles: true, cancelable: true });
    Object.defineProperty(event, 'clipboardData', {
      value: { files: [image, note], getData: () => '', types: ['Files'] },
    });
    markdown.editor.view.dom.dispatchEvent(event);
    expect(onMediaFiles).toHaveBeenCalledWith([image], null);
    expect(markdown.text()).toBe('Text\n');
  });

  it('reads display sizes of the allowed subset only', () => {
    expect(cssSize('50%')).toBe('50%');
    expect(cssSize('320')).toBe('320px');
    expect(cssSize('320px')).toBe('320px');
    expect(cssSize('0')).toBeNull();
    expect(cssSize('150%')).toBeNull();
    expect(cssSize('calc(100% - 1px)')).toBeNull();
    expect(cssSize(null)).toBeNull();
  });
});

describe('media picker', () => {
  const media = [
    medium(IMAGE, 'image', 'Hafen'),
    medium('a1', 'image', 'Altstadt', { archivedAt: 'x' }),
    medium('a2', 'image', 'Berge', { deletedAt: 'x' }),
    medium(PDF, 'pdf', 'Studie', { pageCount: 3 }),
  ];

  it('lists live media of the kind that match every word', () => {
    expect(matchingMedia(media, 'image', '', 'de').map((entry) => entry.id)).toEqual([IMAGE]);
    expect(matchingMedia(media, 'pdf', 'stu', 'de').map((entry) => entry.id)).toEqual([PDF]);
    expect(matchingMedia(media, 'pdf', 'hafen', 'de')).toEqual([]);
  });

  it('takes an empty page as the whole PDF and refuses pages beyond it', () => {
    expect(pageOf('', 3)).toBeNull();
    expect(pageOf(' 2 ', 3)).toBe(2);
    expect(pageOf('4', 3)).toBeNaN();
    expect(pageOf('0', null)).toBeNaN();
    expect(pageOf('zwei', null)).toBeNaN();
    expect(pageOf('120', null)).toBe(120);
  });
});
