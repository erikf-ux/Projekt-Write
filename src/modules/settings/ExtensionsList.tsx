/**
 * Settings › Erweitert: the built-in extensions per registry (ADR 0027,
 * decision 5), read only. The registries of Rust come from
 * `extensions_list`, those of the interface from the sealed registries of
 * the frontend; a kind both sides know (formats, exporters) is listed once.
 */
import { useEffect, useState } from 'react';
import { useT, type MessageKey } from '@/i18n';
import { registries } from '@/extensions/registries';
import { call } from '@/services/ipc';
import type { ExtensionInfo } from '@/types/generated/ExtensionInfo';

const FIELDSET = 'mb-4 rounded-pw bg-sidebar p-4';
const LEGEND = 'text-ui font-semibold tracking-wide text-muted uppercase';

interface Group {
  key: string;
  titleKey: MessageKey;
  entries: { id: string; addOn: boolean }[];
}

/** Ids in order, each once: Rust first, then what only the interface adds. */
function merge(core: readonly ExtensionInfo[], ui: readonly { id: string }[]): Group['entries'] {
  const seen = new Set<string>();
  const entries: Group['entries'] = [];
  for (const entry of [
    ...core.map((e) => ({ id: e.id, addOn: e.addOn })),
    ...ui.map((e) => ({ id: e.id, addOn: false })),
  ]) {
    if (!seen.has(entry.id)) {
      seen.add(entry.id);
      entries.push(entry);
    }
  }
  return entries;
}

export function groupExtensions(core: readonly ExtensionInfo[]): Group[] {
  const of = (registry: ExtensionInfo['registry']) => core.filter((entry) => entry.registry === registry);
  return [
    {
      key: 'editorFeatures',
      titleKey: 'settings.extensions.editorFeatures',
      entries: merge([], registries.editorFeatures.list()),
    },
    {
      key: 'documentFormats',
      titleKey: 'settings.extensions.documentFormats',
      entries: merge(of('documentFormats'), registries.documentFormats.list()),
    },
    {
      key: 'languageEngines',
      titleKey: 'settings.extensions.languageEngines',
      entries: merge(of('languageEngines'), []),
    },
    {
      key: 'exporters',
      titleKey: 'settings.extensions.exporters',
      entries: merge(of('exporters'), registries.exporters.list()),
    },
    {
      key: 'meshRenderers',
      titleKey: 'settings.extensions.meshRenderers',
      entries: merge([], registries.meshRenderers.list()),
    },
  ];
}

export function ExtensionsList() {
  const t = useT();
  const [core, setCore] = useState<readonly ExtensionInfo[]>([]);

  useEffect(() => {
    let current = true;
    void call('extensions_list', {}).then((result) => {
      if (current && result.ok) {
        setCore(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, []);

  return (
    <fieldset className={FIELDSET} data-testid="advanced-extensions">
      <legend className={LEGEND}>{t('settings.extensions.title')}</legend>
      <p className="text-ui mb-2 text-muted">{t('settings.extensions.about')}</p>
      <dl className="flex flex-col gap-2">
        {groupExtensions(core).map((group) => (
          <div key={group.key} data-registry={group.key}>
            <dt className="text-ui font-medium text-fg">{t(group.titleKey)}</dt>
            <dd className="text-ui flex flex-wrap gap-x-3 gap-y-1 text-muted">
              {group.entries.map((entry) => (
                <span key={entry.id} data-extension={entry.id}>
                  <code className="text-fg">{entry.id}</code>{' '}
                  {entry.addOn ? t('settings.extensions.addOn') : t('settings.extensions.builtin')}
                </span>
              ))}
            </dd>
          </div>
        ))}
      </dl>
    </fieldset>
  );
}
