/** Asked when quitting although a document could not be saved. */
import { Dialog } from '@/components/Dialog';
import { useT } from '@/i18n';
import { answerClosePrompt } from './lifecycle';

export function ClosePrompt() {
  const t = useT();
  return (
    <Dialog
      title={t('close.title')}
      onClose={() => void answerClosePrompt(false)}
      testId="close-prompt"
      width="w-[min(460px,92vw)]"
    >
      <p className="text-ui">{t('close.text')}</p>
      <div className="mt-4 flex justify-end gap-2">
        <button
          type="button"
          data-autofocus
          className="text-ui inline-flex h-7 items-center rounded-pw bg-accent px-3 font-medium text-surface"
          onClick={() => void answerClosePrompt(false)}
        >
          {t('close.stay')}
        </button>
        <button
          type="button"
          className="text-ui inline-flex h-7 items-center rounded-pw border border-danger px-3 text-danger"
          onClick={() => void answerClosePrompt(true)}
        >
          {t('close.quit')}
        </button>
      </div>
    </Dialog>
  );
}
