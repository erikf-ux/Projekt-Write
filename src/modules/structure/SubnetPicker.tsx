/**
 * "Verschieben nach …" and "Zuordnen zu …" (ADR 0017, decisions 5 and 14):
 * a list of every subnet with its path, searchable, without the mouse.
 * Moving a document also offers "Nicht zugeordnet", moving a subnet the
 * top level; a subnet is never offered below itself.
 */
import { useMemo, useState, type KeyboardEvent } from 'react';
import { Dialog } from '@/components/Dialog';
import { useLocale, useT } from '@/i18n';
import { ColorDot } from '@/modules/documents/Explorer';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useUi } from '@/modules/settings/uiStore';
import { useStructureDialogs, type PickerState } from './dialogStore';
import { NO_PLACEMENT, useStructure } from './structureStore';
import { buildIndex, inTreeOrder, PATH_SEPARATOR, pathTo, subtree } from './tree';

const SHOWN = 100;
/** The special first entry: "Nicht zugeordnet" or "Oberste Ebene". */
const NONE = '';

export function SubnetPicker() {
  const state = useStructureDialogs((store) => store.picker);
  return state ? <Picker key={JSON.stringify(state)} state={state} /> : null;
}

function Picker({ state }: { state: PickerState }) {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const subnets = useStructure((store) => store.subnets);
  const placement = useStructure((store) =>
    state.mode === 'moveSubnet' ? NO_PLACEMENT : (store.byElement[state.documentId] ?? NO_PLACEMENT),
  );
  const title = useWorkspace((store) =>
    state.mode === 'moveSubnet'
      ? (subnets.find((subnet) => subnet.id === state.subnetId)?.name ?? '')
      : (store.documents.find((document) => document.id === state.documentId)?.title ?? ''),
  );
  const [query, setQuery] = useState('');
  const [current, setCurrent] = useState(0);

  const index = useMemo(() => buildIndex(subnets, locale), [subnets, locale]);
  const entries = useMemo(() => {
    const excluded = state.mode === 'moveSubnet' ? subtree(index, state.subnetId) : new Set<string>();
    const words = query.toLocaleLowerCase(locale).split(/\s+/).filter(Boolean);
    const list = inTreeOrder(index)
      .filter((subnet) => !excluded.has(subnet.id))
      .map((subnet) => ({
        id: subnet.id,
        color: subnet.color,
        path: pathTo(index, subnet.id)
          .map((entry) => entry.name)
          .join(PATH_SEPARATOR),
      }))
      .filter((entry) => words.every((word) => entry.path.toLocaleLowerCase(locale).includes(word)));
    const special =
      state.mode === 'assignDocument'
        ? []
        : [
            {
              id: NONE,
              color: null,
              path: t(state.mode === 'moveSubnet' ? 'structure.topLevel' : 'explorer.group.unassigned'),
            },
          ];
    return [...(words.length === 0 ? special : []), ...list].slice(0, SHOWN);
  }, [index, query, locale, state, t]);

  const close = () => {
    useStructureDialogs.getState().close();
  };
  const choose = async (id: string) => {
    const store = useStructure.getState();
    const target = id === NONE ? null : id;
    const error =
      state.mode === 'moveSubnet'
        ? await store.moveSubnet(state.subnetId, target)
        : state.mode === 'moveDocument'
          ? await store.moveDocument(state.documentId, target)
          : target
            ? await store.assign(state.documentId, target)
            : null;
    close();
    if (error) {
      useUi.getState().showNotice({ tone: 'error', key: 'structure.failed', error });
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'ArrowDown') {
      setCurrent((value) => Math.min(entries.length - 1, value + 1));
    } else if (event.key === 'ArrowUp') {
      setCurrent((value) => Math.max(0, value - 1));
    } else if (event.key === 'Enter') {
      const entry = entries[current];
      if (entry) {
        void choose(entry.id);
      }
    } else {
      return;
    }
    event.preventDefault();
  };

  const heading =
    state.mode === 'moveSubnet'
      ? t('structure.pickMoveSubnet', { name: title })
      : state.mode === 'moveDocument'
        ? t('structure.pickMoveDocument', { title })
        : t('structure.pickAssignDocument', { title });
  const listId = 'pw-subnet-picker-list';
  const optionId = (position: number) => `pw-subnet-picker-${String(position)}`;

  return (
    <Dialog title={heading} onClose={close} testId="subnet-picker" width="w-[min(560px,92vw)]">
      <div className="text-ui flex flex-col gap-2">
        <input
          type="search"
          data-autofocus
          data-testid="subnet-picker-search"
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-activedescendant={entries[current] ? optionId(current) : undefined}
          aria-label={t('structure.pickSearch')}
          placeholder={t('structure.pickSearch')}
          className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setCurrent(0);
          }}
          onKeyDown={onKeyDown}
        />
        <ul id={listId} role="listbox" aria-label={heading} className="max-h-[50vh] overflow-y-auto">
          {entries.map((entry, position) => {
            const member = entry.id !== NONE && placement.subnetIds.includes(entry.id);
            const primary = entry.id !== NONE && placement.subnetIds[0] === entry.id;
            return (
              <li
                key={entry.id === NONE ? 'none' : entry.id}
                id={optionId(position)}
                role="option"
                aria-selected={position === current}
                data-subnet-choice={entry.id === NONE ? 'none' : entry.id}
                className={`flex cursor-default items-center gap-2 rounded-pw px-2 py-1 ${
                  position === current ? 'bg-surface' : ''
                }`}
                onMouseEnter={() => {
                  setCurrent(position);
                }}
                onClick={() => void choose(entry.id)}
              >
                {entry.color ? <ColorDot color={entry.color} /> : <span className="inline-block size-2.5" />}
                <span className="truncate">{entry.path}</span>
                {primary ? (
                  <span className="ml-auto text-muted">{t('structure.isPrimary')}</span>
                ) : member ? (
                  <span className="ml-auto text-muted">{t('structure.isMember')}</span>
                ) : null}
              </li>
            );
          })}
        </ul>
        {entries.length === 0 ? <p className="text-muted">{t('structure.pickNone')}</p> : null}
      </div>
    </Dialog>
  );
}
