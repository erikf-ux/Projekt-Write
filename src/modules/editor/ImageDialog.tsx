/**
 * "Bild bearbeiten" (section 12; ADR 0019, decision 10): alternative text,
 * caption (the image title, shown below the picture) and display size. The
 * size is part of the allowed HTML subset (`<img … width="50%">`) and only
 * changes how the image is shown; the medium itself stays as it is.
 */
import { NodeSelection } from '@tiptap/pm/state';
import { useId, useState } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { useT } from '@/i18n';
import { getSession } from './session';

/** Display sizes offered besides the original size. */
export const IMAGE_SIZES = ['25%', '50%', '75%', '100%'] as const;

interface ImageDialogStore {
  request: { tab: string; position: number } | null;
  open: (tab: string, position: number) => void;
  close: () => void;
}

export const useImageDialog = create<ImageDialogStore>((set) => ({
  request: null,
  open: (tab, position) => {
    set({ request: { tab, position } });
  },
  close: () => {
    set({ request: null });
  },
}));

interface ImageFields {
  alt: string;
  caption: string;
  /** `''` for the original size. */
  size: string;
}

/** The attributes of the image node at `position`, or null if there is none. */
function imageAt(tab: string, position: number): ImageFields | null {
  const node = getSession(tab)?.editor.state.doc.nodeAt(position);
  if (node?.type.name !== 'image') {
    return null;
  }
  const text = (name: string) => {
    const value: unknown = node.attrs[name];
    return typeof value === 'string' ? value : '';
  };
  return { alt: text('alt'), caption: text('title'), size: text('width') || text('height') };
}

/**
 * The image "Bild bearbeiten" works on: the selected one, or the one right
 * after (then before) an empty cursor. WebKitGTK turns the selection of a
 * picture back into a cursor next to it, so the cursor counts as well.
 */
export function selectedImage(tab: string): number | null {
  const selection = getSession(tab)?.editor.state.selection;
  if (!selection) {
    return null;
  }
  if (selection instanceof NodeSelection) {
    return selection.node.type.name === 'image' ? selection.from : null;
  }
  if (!selection.empty) {
    return null;
  }
  const { $from } = selection;
  if ($from.nodeAfter?.type.name === 'image') {
    return $from.pos;
  }
  if ($from.nodeBefore?.type.name === 'image') {
    return $from.pos - $from.nodeBefore.nodeSize;
  }
  return null;
}

export function ImageDialog() {
  const request = useImageDialog((state) => state.request);
  if (!request) {
    return null;
  }
  const fields = imageAt(request.tab, request.position);
  return fields ? (
    <ImageForm
      key={`${request.tab}:${String(request.position)}`}
      tab={request.tab}
      position={request.position}
      initial={fields}
    />
  ) : null;
}

function ImageForm({ tab, position, initial }: { tab: string; position: number; initial: ImageFields }) {
  const t = useT();
  const [fields, setFields] = useState(initial);
  const altId = useId();
  const captionId = useId();
  const sizeId = useId();
  const sizes: string[] = [...IMAGE_SIZES];
  if (initial.size !== '' && !sizes.includes(initial.size)) {
    sizes.unshift(initial.size);
  }

  const close = () => {
    useImageDialog.getState().close();
    getSession(tab)?.editor.commands.focus();
  };
  const apply = () => {
    const editor = getSession(tab)?.editor;
    useImageDialog.getState().close();
    if (editor?.state.doc.nodeAt(position)?.type.name !== 'image') {
      return;
    }
    const sizeChanged = fields.size !== initial.size;
    editor
      .chain()
      .focus()
      .setNodeSelection(position)
      .updateAttributes('image', {
        alt: fields.alt.trim(),
        title: fields.caption.trim() === '' ? null : fields.caption.trim(),
        // A new size is a width; the height then follows the picture.
        ...(sizeChanged ? { width: fields.size === '' ? null : fields.size, height: null } : {}),
      })
      .run();
  };
  const field = 'h-8 rounded-pw border border-muted bg-surface px-2 text-fg';
  return (
    <Dialog title={t('editor.image.edit')} onClose={close} testId="image-dialog" width="w-[min(480px,92vw)]">
      <form
        className="text-ui flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          apply();
        }}
      >
        <label htmlFor={altId} className="flex flex-col gap-1">
          {t('editor.image.alt')}
          <input
            id={altId}
            type="text"
            className={field}
            value={fields.alt}
            maxLength={500}
            autoFocus
            data-autofocus
            data-testid="image-alt"
            onChange={(event) => {
              setFields({ ...fields, alt: event.target.value });
            }}
          />
        </label>
        <label htmlFor={captionId} className="flex flex-col gap-1">
          {t('editor.image.caption')}
          <input
            id={captionId}
            type="text"
            className={field}
            value={fields.caption}
            maxLength={500}
            data-testid="image-caption"
            onChange={(event) => {
              setFields({ ...fields, caption: event.target.value });
            }}
          />
        </label>
        <label htmlFor={sizeId} className="flex flex-col gap-1">
          {t('editor.image.size')}
          <select
            id={sizeId}
            className={field}
            value={fields.size}
            data-testid="image-size"
            onChange={(event) => {
              setFields({ ...fields, size: event.target.value });
            }}
          >
            <option value="">{t('editor.image.sizeOriginal')}</option>
            {sizes.map((size) => (
              <option key={size} value={size}>
                {size}
              </option>
            ))}
          </select>
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" className="h-7 rounded-pw border border-muted px-3" onClick={close}>
            {t('media.cancel')}
          </button>
          <button
            type="submit"
            className="h-7 rounded-pw bg-accent px-3 font-medium text-surface"
            data-testid="image-apply"
          >
            {t('editor.image.apply')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
