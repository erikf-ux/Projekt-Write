/**
 * Setting or removing a web link (section 12: "Erlaubt sind nur `http`,
 * `https` und `mailto`"). Relative addresses (`kapitel-2.md`, `#teil`) are
 * allowed as well: they stay in the Markdown file and open nothing.
 */
import { useState } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { useT } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { getSession } from './session';

interface LinkDialogStore {
  /** The current address, or null while the dialog is closed. */
  href: string | null;
  open: (href: string) => void;
  close: () => void;
}

export const useLinkDialog = create<LinkDialogStore>((set) => ({
  href: null,
  open: (href) => {
    set({ href });
  },
  close: () => {
    set({ href: null });
  },
}));

/** True for http, https and mailto addresses and for relative ones. */
export function isAllowedLink(href: string): boolean {
  const value = href.trim();
  if (value === '' || /[\s<>\p{Cc}]/u.test(value)) {
    return false;
  }
  const scheme = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(value)?.[1]?.toLowerCase();
  return scheme === undefined || scheme === 'http' || scheme === 'https' || scheme === 'mailto';
}

export function LinkDialog() {
  const initial = useLinkDialog((state) => state.href);
  return initial === null ? null : <LinkForm initial={initial} />;
}

function LinkForm({ initial }: { initial: string }) {
  const t = useT();
  const [href, setHref] = useState(initial);
  const [touched, setTouched] = useState(false);
  const valid = isAllowedLink(href);
  const editor = () => {
    const id = useWorkspace.getState().active;
    return id ? (getSession(id)?.editor ?? null) : null;
  };
  const close = () => {
    useLinkDialog.getState().close();
    editor()?.commands.focus();
  };
  const apply = () => {
    setTouched(true);
    if (!valid) {
      return;
    }
    const target = editor();
    const value = href.trim();
    if (target) {
      const chain = target.chain().focus().extendMarkRange('link');
      if (target.state.selection.empty && !target.isActive('link')) {
        chain
          .insertContent({ type: 'text', text: value, marks: [{ type: 'link', attrs: { href: value } }] })
          .run();
      } else {
        chain.setLink({ href: value }).run();
      }
    }
    useLinkDialog.getState().close();
  };
  const remove = () => {
    editor()?.chain().focus().extendMarkRange('link').unsetLink().run();
    useLinkDialog.getState().close();
  };
  return (
    <Dialog title={t('link.title')} onClose={close} testId="link-dialog" width="w-[min(480px,92vw)]">
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          apply();
        }}
      >
        <label className="text-ui flex flex-col gap-1">
          {t('link.address')}
          <input
            type="text"
            className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg aria-[invalid=true]:border-danger"
            value={href}
            autoFocus
            aria-invalid={touched && !valid}
            aria-describedby="pw-link-hint"
            data-testid="link-address"
            onChange={(event) => {
              setHref(event.target.value);
            }}
          />
        </label>
        <p id="pw-link-hint" className={`text-ui ${touched && !valid ? 'text-danger' : 'text-muted'}`}>
          {t('link.hint')}
        </p>
        <div className="flex justify-end gap-2">
          {initial !== '' ? (
            <button
              type="button"
              className="text-ui h-7 rounded-pw border border-muted px-3"
              onClick={remove}
            >
              {t('link.remove')}
            </button>
          ) : null}
          <button type="button" className="text-ui h-7 rounded-pw border border-muted px-3" onClick={close}>
            {t('link.cancel')}
          </button>
          <button type="submit" className="text-ui h-7 rounded-pw bg-accent px-3 font-medium text-surface">
            {t('link.apply')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
