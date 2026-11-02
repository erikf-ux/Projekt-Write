import { CircleCheck, CircleX, RefreshCw } from 'lucide-react';
import { useEffect } from 'react';
import { isMessageKey, useT, type MessageKey } from '@/i18n';
import type { SystemCheckId } from '@/types/generated/SystemCheckId';
import { Logo } from './Logo';
import { useSystemStatus } from './systemStatusStore';

const checkLabels: Record<SystemCheckId, MessageKey> = {
  database: 'check.database',
  migrations: 'check.migrations',
  foreignKeys: 'check.foreignKeys',
  fullTextSearch: 'check.fullTextSearch',
};

const platformLabels: Record<string, MessageKey> = {
  windows: 'platform.windows',
  macos: 'platform.macos',
  linux: 'platform.linux',
};

export function StartScreen() {
  const t = useT();
  const status = useSystemStatus((state) => state.status);
  const run = useSystemStatus((state) => state.run);

  useEffect(() => {
    void run();
  }, [run]);

  let summary: string;
  let summaryTone = 'text-muted';
  if (status.phase === 'running') {
    summary = t('start.checks.running');
  } else if (status.phase === 'error') {
    const key = status.error.messageKey;
    summary = t(isMessageKey(key) ? key : 'error.internal');
    summaryTone = 'text-danger';
  } else if (status.response.results.every((result) => result.passed)) {
    summary = t('start.checks.allPassed');
  } else {
    summary = t('start.checks.someFailed');
    summaryTone = 'text-danger';
  }

  const response = status.phase === 'done' ? status.response : undefined;

  return (
    <main
      aria-labelledby="app-title"
      className="flex min-h-full flex-col items-center justify-center bg-surface px-6 py-10 text-fg"
    >
      <div className="w-full max-w-[560px]">
        <header className="mb-8 flex items-center gap-4">
          <Logo className="size-12 shrink-0" />
          <div>
            <h1 id="app-title" className="text-[24px] leading-tight font-semibold">
              {t('app.name')}
            </h1>
            <p className="text-ui text-muted">{t('start.tagline')}</p>
          </div>
        </header>

        <section aria-labelledby="checks-title" className="rounded-pw bg-sidebar p-4" data-testid="system-check">
          <h2 id="checks-title" className="text-ui mb-2 font-semibold tracking-wide text-muted uppercase">
            {t('start.checks.title')}
          </h2>

          <p role="status" aria-live="polite" className={`text-ui mb-3 ${summaryTone}`} data-testid="check-summary">
            {summary}
          </p>

          {response && (
            <ul className="mb-4 flex flex-col gap-1">
              {response.results.map((result) => (
                <li
                  key={result.check}
                  className="text-ui flex items-center gap-2"
                  data-check={result.check}
                  data-passed={result.passed}
                >
                  {result.passed ? (
                    <CircleCheck aria-hidden="true" className="size-4 shrink-0 text-success" />
                  ) : (
                    <CircleX aria-hidden="true" className="size-4 shrink-0 text-danger" />
                  )}
                  <span className="flex-1">{t(checkLabels[result.check])}</span>
                  <span className={result.passed ? 'text-success' : 'text-danger'}>
                    {t(result.passed ? 'start.checks.passed' : 'start.checks.failed')}
                  </span>
                </li>
              ))}
            </ul>
          )}

          <button
            type="button"
            onClick={() => void run()}
            disabled={status.phase === 'running'}
            className="text-ui inline-flex h-7 items-center gap-2 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-60"
          >
            <RefreshCw aria-hidden="true" className="size-3.5" />
            {t('start.checks.retry')}
          </button>
        </section>

        {response && (
          <footer className="text-ui mt-4 flex flex-wrap gap-x-3 text-muted" data-testid="start-footer">
            <span>{t('start.footer.version', { version: response.appVersion })}</span>
            <span aria-hidden="true">·</span>
            <span>{t(platformLabels[response.platform] ?? 'platform.unknown')}</span>
            <span aria-hidden="true">·</span>
            <span>{t('start.footer.sqlite', { version: response.sqliteVersion })}</span>
            <span aria-hidden="true">·</span>
            <span>{t('start.footer.schema', { version: response.schemaVersion })}</span>
          </footer>
        )}
      </div>
    </main>
  );
}
