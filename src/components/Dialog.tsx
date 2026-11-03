/**
 * Modal dialog: the focus stays inside (Tab and Shift+Tab wrap around),
 * Escape closes, and the focus returns to where it was before.
 */
import { X } from 'lucide-react';
import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { useT } from '@/i18n';

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  testId?: string;
  /** Escape is handled by the content (e.g. while recording a shortcut). */
  escapeDisabled?: boolean;
  width?: string;
}

/** Open dialogs in the order they opened. */
const openDialogs: HTMLElement[] = [];

/**
 * The dialog that handles Escape and the focus: the last opened one that
 * contains no other open dialog (nested dialogs mount child first).
 */
function topDialog(): HTMLElement | undefined {
  const innermost = openDialogs.filter(
    (dialog) => !openDialogs.some((other) => other !== dialog && dialog.contains(other)),
  );
  return innermost[innermost.length - 1];
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Dialog({
  title,
  onClose,
  children,
  testId,
  escapeDisabled = false,
  width = 'w-[min(720px,92vw)]',
}: DialogProps) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const element = panel.current;
    const focusFirst = () => {
      const first =
        element?.querySelector<HTMLElement>('[data-autofocus]') ??
        element?.querySelector<HTMLElement>(FOCUSABLE);
      (first ?? element)?.focus();
    };
    focusFirst();
    if (element) {
      openDialogs.push(element);
    }
    const isTop = () => element !== null && topDialog() === element;
    // The dialog is modal: focus that leaves it (or falls back to the page
    // because the focused control disappeared) comes back.
    const onFocusIn = (event: FocusEvent) => {
      if (isTop() && element && event.target instanceof Node && !element.contains(event.target)) {
        focusFirst();
      }
    };
    document.addEventListener('focusin', onFocusIn);
    return () => {
      document.removeEventListener('focusin', onFocusIn);
      const index = element ? openDialogs.indexOf(element) : -1;
      if (index !== -1) {
        openDialogs.splice(index, 1);
      }
      previous?.focus();
    };
  }, []);

  // Escape closes the innermost dialog wherever the focus is (also on the
  // page itself after a control inside disappeared).
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      const element = panel.current;
      if (
        event.key !== 'Escape' ||
        event.defaultPrevented ||
        escapeDisabled ||
        !element ||
        topDialog() !== element
      ) {
        return;
      }
      event.preventDefault();
      onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [escapeDisabled, onClose]);

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== 'Tab' || !panel.current) {
      return;
    }
    const focusable = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (element) => element.offsetParent !== null || element === document.activeElement,
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) {
      return;
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-black/30 py-[8vh]">
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        data-testid={testId}
        className={`${width} rounded-pw border border-muted bg-sidebar text-fg shadow-lg`}
        onKeyDown={onKeyDown}
      >
        <header className="flex items-center justify-between border-b border-muted/40 px-4 py-2">
          <h2 id={titleId} className="text-[15px] font-semibold">
            {title}
          </h2>
          <button
            type="button"
            aria-label={t('dialog.close')}
            className="inline-flex size-7 items-center justify-center rounded-pw text-muted hover:bg-surface hover:text-fg"
            onClick={onClose}
          >
            <X aria-hidden="true" className="size-4" />
          </button>
        </header>
        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}
