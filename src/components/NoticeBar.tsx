/**
 * Short messages at the bottom of the window. Errors stay until closed
 * (role "alert"); information disappears after a few seconds.
 */
import { X } from 'lucide-react';
import { useEffect } from 'react';
import { useT } from '@/i18n';
import { errorText } from './errorText';
import type { Notice } from '@/modules/settings/uiStore';

interface NoticeBarProps {
  notice: Notice | null;
  onClose: () => void;
}

const INFO_TIMEOUT_MS = 6000;

export function NoticeBar({ notice, onClose }: NoticeBarProps) {
  const t = useT();
  useEffect(() => {
    if (notice?.tone !== 'info') {
      return;
    }
    const timer = window.setTimeout(onClose, INFO_TIMEOUT_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [notice, onClose]);
  if (!notice) {
    return null;
  }
  return (
    <div
      role={notice.tone === 'error' ? 'alert' : 'status'}
      data-testid="notice"
      data-tone={notice.tone}
      className={`text-ui fixed right-4 bottom-8 z-30 flex max-w-[min(520px,90vw)] items-start gap-2 rounded-pw border bg-sidebar px-3 py-2 text-fg shadow-lg ${
        notice.tone === 'error' ? 'border-danger' : 'border-muted'
      }`}
    >
      <div className="flex-1">
        <p>{t(notice.key, notice.params)}</p>
        {notice.error ? <p className="text-danger">{errorText(t, notice.error)}</p> : null}
      </div>
      <button
        type="button"
        aria-label={t('dialog.close')}
        className="inline-flex size-6 shrink-0 items-center justify-center rounded-pw text-muted hover:bg-surface hover:text-fg"
        onClick={onClose}
      >
        <X aria-hidden="true" className="size-3.5" />
      </button>
    </div>
  );
}
