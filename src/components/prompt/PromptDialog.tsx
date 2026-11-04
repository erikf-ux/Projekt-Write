/** The first waiting question of `ask`. */
import { Dialog } from '@/components/Dialog';
import { useT } from '@/i18n';
import { usePrompt } from './promptStore';

const TONES = {
  primary: 'bg-accent font-medium text-surface',
  danger: 'border border-danger text-danger',
  default: 'border border-muted',
} as const;

export function PromptDialog() {
  const t = useT();
  const pending = usePrompt((state) => state.queue[0]);
  const answer = usePrompt((state) => state.answer);
  if (!pending) {
    return null;
  }
  const { request } = pending;
  return (
    <Dialog
      title={t(request.titleKey, request.params)}
      onClose={() => {
        answer(request.cancel);
      }}
      testId={request.testId ?? 'prompt'}
      width="w-[min(480px,92vw)]"
    >
      <p className="text-ui">{t(request.textKey, request.params)}</p>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        {request.choices.map((choice, index) => (
          <button
            key={choice.id}
            type="button"
            data-autofocus={index === 0 ? true : undefined}
            data-choice={choice.id}
            className={`text-ui inline-flex h-7 items-center rounded-pw px-3 ${TONES[choice.tone ?? 'default']}`}
            onClick={() => {
              answer(choice.id);
            }}
          >
            {t(choice.labelKey, request.params)}
          </button>
        ))}
      </div>
    </Dialog>
  );
}
