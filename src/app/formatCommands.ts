/**
 * Editing and formatting commands of Phase 4 (section 12, ADR 0012): the
 * Edit and Format menus, the command palette and the shortcuts all run
 * them. Formatting works in the rich text of Markdown documents; in the
 * source view and in plain text it is not offered.
 */
import type { Editor } from '@tiptap/core';
import type { CommandDefinition, CommandId } from '@/modules/commands/types';
import { activeDocument, useWorkspace } from '@/modules/documents/workspaceStore';
import { selectedImage, useImageDialog } from '@/modules/editor/ImageDialog';
import { useLinkDialog } from '@/modules/editor/LinkDialog';
import { useMediaPicker } from '@/modules/editor/MediaPicker';
import { getSession } from '@/modules/editor/session';
import { useLibrary } from '@/modules/library/libraryStore';

const libraryOpen = () => useLibrary.getState().status?.state === 'open';

function activeEditor(): Editor | null {
  const id = useWorkspace.getState().active;
  return id ? (getSession(id)?.editor ?? null) : null;
}

/** The rich editor of the active tab, if it can be edited. */
function richEditor(): Editor | null {
  const document = activeDocument(useWorkspace.getState());
  if (document?.loaded !== true || document.view !== 'rich' || !libraryOpen()) {
    return null;
  }
  return activeEditor();
}

function format(
  id: CommandId,
  titleKey: CommandDefinition['titleKey'],
  apply: (editor: Editor) => void,
): CommandDefinition {
  return {
    id,
    category: 'edit',
    titleKey,
    isAvailable: () => richEditor() !== null,
    run: () => {
      const editor = richEditor();
      if (editor) {
        apply(editor);
      }
    },
  };
}

export function createFormatCommands(): CommandDefinition[] {
  return [
    {
      id: 'edit.undo',
      category: 'edit',
      titleKey: 'command.edit.undo',
      isAvailable: () => activeEditor() !== null && libraryOpen(),
      run: () => {
        activeEditor()?.chain().focus().undo().run();
      },
    },
    {
      id: 'edit.redo',
      category: 'edit',
      titleKey: 'command.edit.redo',
      isAvailable: () => activeEditor() !== null && libraryOpen(),
      run: () => {
        activeEditor()?.chain().focus().redo().run();
      },
    },
    format('format.bold', 'command.format.bold', (editor) => editor.chain().focus().toggleBold().run()),
    format('format.italic', 'command.format.italic', (editor) => editor.chain().focus().toggleItalic().run()),
    format('format.underline', 'command.format.underline', (editor) =>
      editor.chain().focus().toggleUnderline().run(),
    ),
    format('format.strike', 'command.format.strike', (editor) => editor.chain().focus().toggleStrike().run()),
    format('format.code', 'command.format.code', (editor) => editor.chain().focus().toggleCode().run()),
    format('format.paragraph', 'command.format.paragraph', (editor) =>
      editor.chain().focus().setParagraph().run(),
    ),
    format('format.heading1', 'command.format.heading1', (editor) =>
      editor.chain().focus().toggleHeading({ level: 1 }).run(),
    ),
    format('format.heading2', 'command.format.heading2', (editor) =>
      editor.chain().focus().toggleHeading({ level: 2 }).run(),
    ),
    format('format.heading3', 'command.format.heading3', (editor) =>
      editor.chain().focus().toggleHeading({ level: 3 }).run(),
    ),
    format('format.bulletList', 'command.format.bulletList', (editor) =>
      editor.chain().focus().toggleBulletList().run(),
    ),
    format('format.orderedList', 'command.format.orderedList', (editor) =>
      editor.chain().focus().toggleOrderedList().run(),
    ),
    format('format.taskList', 'command.format.taskList', (editor) =>
      editor.chain().focus().toggleTaskList().run(),
    ),
    format('format.blockquote', 'command.format.blockquote', (editor) =>
      editor.chain().focus().toggleBlockquote().run(),
    ),
    format('format.codeBlock', 'command.format.codeBlock', (editor) =>
      editor.chain().focus().toggleCodeBlock().run(),
    ),
    format('format.link', 'command.format.link', (editor) => {
      const href: unknown = editor.getAttributes('link')['href'];
      useLinkDialog.getState().open(typeof href === 'string' ? href : '');
    }),
    // Media of the library (Phase 8): the picker inserts at the cursor.
    format('format.image', 'command.format.image', () => {
      const tab = useWorkspace.getState().active;
      if (tab) {
        useMediaPicker.getState().open(tab, 'image');
      }
    }),
    format('format.pdfReference', 'command.format.pdfReference', () => {
      const tab = useWorkspace.getState().active;
      if (tab) {
        useMediaPicker.getState().open(tab, 'pdf');
      }
    }),
    {
      id: 'format.imageEdit',
      category: 'edit',
      titleKey: 'command.format.imageEdit',
      isAvailable: () => {
        const tab = useWorkspace.getState().active;
        return richEditor() !== null && tab !== null && selectedImage(tab) !== null;
      },
      run: () => {
        const tab = useWorkspace.getState().active;
        const position = tab && richEditor() ? selectedImage(tab) : null;
        if (tab && position !== null) {
          useImageDialog.getState().open(tab, position);
        }
      },
    },
    format('format.horizontalRule', 'command.format.horizontalRule', (editor) =>
      editor.chain().focus().setHorizontalRule().run(),
    ),
    format('format.table', 'command.format.table', (editor) =>
      editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
    ),
  ];
}
