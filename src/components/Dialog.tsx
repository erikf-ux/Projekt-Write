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
    const first =
      panel.current?.querySelector<HTMLElement>('[data-autofocus]') ??
      panel.current?.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    return () => {
      previous?.focus();
    };
  }, []);

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && !escapeDisabled) {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
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
