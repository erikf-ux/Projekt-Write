/**
 * The suggestions beside the Mesh (section 15): each with its reasons and
 * the three decisions. Nothing is applied without the user.
 */
import { type MessageKey, useT } from '@/i18n';
import type { MeshSuggestion } from '@/types/generated/MeshSuggestion';
import { useSuggestions } from './suggestionStore';

function reasons(t: ReturnType<typeof useT>, suggestion: MeshSuggestion): string {
  const { reason } = suggestion;
  const parts: string[] = [];
  if (reason.sharedTags > 0) {
    parts.push(t('mesh.reason.tags', { count: reason.sharedTags }));
  }
  if (reason.content > 0) {
    parts.push(t('mesh.reason.content', { value: reason.content.toFixed(2) }));
  }
  if (reason.title > 0.2) {
    parts.push(t('mesh.reason.title', { value: reason.title.toFixed(2) }));
  }
  if (reason.linked) {
    parts.push(t('mesh.reason.linked'));
  } else if (reason.sharedLinks > 0) {
    parts.push(t('mesh.reason.links', { count: reason.sharedLinks }));
  }
  if (reason.sameFolder) {
    parts.push(t('mesh.reason.folder'));
  }
  if (reason.sharedSubnets > 0) {
    parts.push(t('mesh.reason.subnets', { count: reason.sharedSubnets }));
  }
  return parts.join(', ');
}

export function SuggestionsPanel() {
  const t = useT();
  const store = useSuggestions();
  const shown = store.suggestions.filter((suggestion) => !store.later.includes(suggestion.id));
  const step = store.progress ? t(`mesh.analysisStep.${store.progress.step}` as MessageKey) : '';
  return (
    <aside
      className="flex w-80 shrink-0 flex-col border-l border-muted/40 bg-sidebar text-xs"
      aria-label={t('mesh.suggestions')}
      data-testid="mesh-suggestions"
    >
      <div className="flex flex-col gap-2 border-b border-muted/40 p-2">
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="rounded bg-accent px-2 py-1 text-surface disabled:opacity-50"
            data-testid="mesh-analyze"
            disabled={store.running}
            onClick={() => void store.analyze()}
          >
            {t('mesh.analyze')}
          </button>
          {store.running ? (
            <button
              type="button"
              className="rounded px-2 py-1 hover:bg-muted/30"
              data-testid="mesh-analysis-cancel"
              onClick={() => void store.cancel()}
            >
              {t('mesh.analysisCancel')}
            </button>
          ) : null}
        </div>
        {store.running ? (
          <p role="status" data-testid="mesh-analysis-progress">
            {store.progress
              ? t('mesh.analysisProgress', { step, done: store.progress.done, total: store.progress.total })
              : t('mesh.analysisStarting')}
          </p>
        ) : null}
        <label className="flex items-center gap-2">
          {t('mesh.threshold')}
          <input
            type="number"
            min={0.05}
            max={0.95}
            step={0.05}
            className="w-16 rounded border border-muted/40 bg-surface px-1"
            data-testid="mesh-threshold"
            value={store.threshold}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value) && value >= 0.05 && value <= 0.95) {
                void store.setThreshold(value);
              }
            }}
          />
        </label>
      </div>
      {shown.length === 0 ? (
        <p className="p-2 text-muted" data-testid="mesh-no-suggestions">
          {t('mesh.noSuggestions')}
        </p>
      ) : (
        <ul className="min-h-0 flex-1 overflow-y-auto" data-testid="mesh-suggestion-list">
          {shown.map((suggestion) => (
            <li
              key={suggestion.id}
              className="border-b border-muted/20 p-2"
              data-suggestion={suggestion.id}
              data-nodes={`${suggestion.nodeA} ${suggestion.nodeB}`}
            >
              <p className="font-semibold">
                {suggestion.titleA} – {suggestion.titleB}
              </p>
              <p className="text-muted">
                {t('mesh.score', { value: suggestion.score.toFixed(2) })}: {reasons(t, suggestion)}
              </p>
              <div className="mt-1 flex gap-1">
                <button
                  type="button"
                  className="rounded border border-muted/40 px-1.5 py-0.5 hover:bg-muted/30"
                  data-testid="suggestion-accept"
                  onClick={() => void store.decide(suggestion.id, 'accept')}
                >
                  {t('mesh.accept')}
                </button>
                <button
                  type="button"
                  className="rounded border border-muted/40 px-1.5 py-0.5 hover:bg-muted/30"
                  data-testid="suggestion-reject"
                  onClick={() => void store.decide(suggestion.id, 'reject')}
                >
                  {t('mesh.reject')}
                </button>
                <button
                  type="button"
                  className="rounded px-1.5 py-0.5 hover:bg-muted/30"
                  data-testid="suggestion-later"
                  onClick={() => {
                    store.postpone(suggestion.id);
                  }}
                >
                  {t('mesh.later')}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
