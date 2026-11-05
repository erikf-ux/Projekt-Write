/**
 * Creating and editing a subnet (section 16): name, description, colour.
 * Colours are the five cluster colours of the design, or a colour of one's
 * own that reaches 3:1 against writing surface and side bar of both
 * designs (section 6; ADR 0017, decision 9). The name is always shown with
 * the colour, which never carries meaning alone.
 */
import { useId, useState } from 'react';
import { Dialog } from '@/components/Dialog';
import { errorText } from '@/components/errorText';
import { useT } from '@/i18n';
import type { CommandError } from '@/types/generated/CommandError';
import { ColorDot } from '@/modules/documents/Explorer';
import { useExplorer } from '@/modules/documents/explorerStore';
import { useStructureDialogs, type SubnetDialogState } from './dialogStore';
import { useStructure } from './structureStore';
import {
  buildIndex,
  CLUSTER_COLORS,
  isClusterColor,
  lowestContrast,
  MIN_COLOR_CONTRAST,
  normalizeHex,
  pathLabel,
} from './tree';
import { useLocale } from '@/i18n';

const MAX_NAME = 200;
const MAX_DESCRIPTION = 2000;

export function SubnetDialog() {
  const state = useStructureDialogs((store) => store.subnet);
  // A new key per dialog: the form starts from the subnet each time.
  return state ? <SubnetForm key={JSON.stringify(state)} state={state} /> : null;
}

function SubnetForm({ state }: { state: SubnetDialogState }) {
  const t = useT();
  const locale = useLocale((store) => store.locale);
  const subnets = useStructure((store) => store.subnets);
  const editing = state.mode === 'edit' ? subnets.find((subnet) => subnet.id === state.id) : undefined;
  const parentId = state.mode === 'create' ? state.parentId : (editing?.parentId ?? null);
  const parent = parentId ? subnets.find((subnet) => subnet.id === parentId) : undefined;
  const [name, setName] = useState(editing?.name ?? '');
  const [description, setDescription] = useState(editing?.description ?? '');
  const initialColor = editing?.color ?? parent?.color ?? null;
  const [choice, setChoice] = useState<string | null>(
    initialColor === null ? null : isClusterColor(initialColor) ? initialColor : 'custom',
  );
  const [custom, setCustom] = useState(
    initialColor && !isClusterColor(initialColor) ? initialColor : '#d9480f',
  );
  const [error, setError] = useState<CommandError | null>(null);
  const [busy, setBusy] = useState(false);
  const nameId = useId();
  const descriptionId = useId();
  const customId = useId();

  const close = () => {
    useStructureDialogs.getState().close();
  };
  const hex = normalizeHex(custom);
  const contrast = hex ? lowestContrast(hex) : 0;
  const customOk = hex !== null && contrast >= MIN_COLOR_CONTRAST;
  const color = choice === 'custom' ? (customOk ? hex : null) : choice;
  const canSave = name.trim() !== '' && !(choice === 'custom' && !customOk) && !busy;

  const save = async () => {
    setBusy(true);
    const store = useStructure.getState();
    if (state.mode === 'create') {
      const result = await store.createSubnet({
        name,
        ...(parentId ? { parentId } : {}),
        ...(color ? { color } : {}),
        description,
      });
      setBusy(false);
      if (result.error) {
        setError(result.error);
        return;
      }
      // The new subnet shows in the Explorer, its parent open.
      if (result.id) {
        const index = buildIndex(useStructure.getState().subnets, locale);
        const path = [] as string[];
        let current = parentId;
        while (current) {
          path.unshift(current);
          current = index.byId.get(current)?.parentId ?? null;
        }
        useExplorer.getState().revealRow(`s:${result.id}`, path);
      }
      close();
      return;
    }
    const result = await store.updateSubnet({
      id: state.id,
      name,
      description,
      ...(color ? { color } : {}),
    });
    setBusy(false);
    if (result) {
      setError(result);
      return;
    }
    close();
  };

  const title =
    state.mode === 'edit'
      ? t('subnet.editTitle', { name: editing?.name ?? '' })
      : parent
        ? t('subnet.createChildTitle', { parent: pathLabel(buildIndex(subnets, locale), parent.id) })
        : t('subnet.createTitle');

  return (
    <Dialog title={title} onClose={close} testId="subnet-dialog" width="w-[min(480px,92vw)]">
      <form
        className="text-ui flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          if (canSave) {
            void save();
          }
        }}
      >
        <label htmlFor={nameId} className="flex flex-col gap-1">
          {t('subnet.name')}
          <input
            id={nameId}
            type="text"
            data-autofocus
            data-testid="subnet-name"
            maxLength={MAX_NAME}
            className="text-ui h-8 rounded-pw border border-muted bg-surface px-2 text-fg"
            value={name}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
        </label>
        <label htmlFor={descriptionId} className="flex flex-col gap-1">
          {t('subnet.description')}
          <textarea
            id={descriptionId}
            data-testid="subnet-description"
            maxLength={MAX_DESCRIPTION}
            rows={3}
            className="text-ui rounded-pw border border-muted bg-surface px-2 py-1 text-fg"
            value={description}
            onChange={(event) => {
              setDescription(event.target.value);
            }}
          />
        </label>
        <fieldset className="flex flex-col gap-1">
          <legend className="mb-1">{t('subnet.color')}</legend>
          <div className="flex flex-wrap gap-x-3 gap-y-1" role="radiogroup" aria-label={t('subnet.color')}>
            {state.mode === 'create' ? (
              <label className="inline-flex items-center gap-1.5">
                <input
                  type="radio"
                  name="subnet-color"
                  checked={choice === null}
                  onChange={() => {
                    setChoice(null);
                  }}
                />
                {parent ? t('subnet.colorInherit') : t('subnet.colorNext')}
              </label>
            ) : null}
            {CLUSTER_COLORS.map((cluster) => (
              <label key={cluster} className="inline-flex items-center gap-1.5">
                <input
                  type="radio"
                  name="subnet-color"
                  data-color-choice={cluster}
                  checked={choice === cluster}
                  onChange={() => {
                    setChoice(cluster);
                  }}
                />
                <ColorDot color={cluster} />
                {t(`subnet.colors.${cluster}`)}
              </label>
            ))}
            <label className="inline-flex items-center gap-1.5">
              <input
                type="radio"
                name="subnet-color"
                data-color-choice="custom"
                checked={choice === 'custom'}
                onChange={() => {
                  setChoice('custom');
                }}
              />
              {t('subnet.colorCustom')}
            </label>
          </div>
          {choice === 'custom' ? (
            <div className="flex flex-col gap-1 pt-1">
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  aria-label={t('subnet.colorPicker')}
                  className="h-7 w-10 rounded-pw border border-muted bg-surface"
                  value={hex ?? '#000000'}
                  onChange={(event) => {
                    setCustom(event.target.value);
                  }}
                />
                <label htmlFor={customId} className="sr-only">
                  {t('subnet.colorHex')}
                </label>
                <input
                  id={customId}
                  type="text"
                  data-testid="subnet-color-hex"
                  className="text-ui h-7 w-24 rounded-pw border border-muted bg-surface px-2 font-mono text-fg"
                  value={custom}
                  maxLength={7}
                  onChange={(event) => {
                    setCustom(event.target.value);
                  }}
                />
                {hex ? <ColorDot color={hex} /> : null}
              </div>
              <p
                className={customOk ? 'text-muted' : 'text-danger'}
                data-testid="subnet-color-contrast"
                role={customOk ? undefined : 'alert'}
              >
                {hex === null
                  ? t('subnet.colorFormat')
                  : customOk
                    ? t('subnet.colorContrastOk', { ratio: contrast.toFixed(1) })
                    : t('subnet.colorContrastLow', { ratio: contrast.toFixed(1) })}
              </p>
            </div>
          ) : null}
        </fieldset>
        {error ? (
          <p role="alert" className="text-danger" data-testid="subnet-error">
            {errorText(t, error)}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <button type="button" className="text-ui h-7 rounded-pw border border-muted px-3" onClick={close}>
            {t('subnet.cancel')}
          </button>
          <button
            type="submit"
            data-testid="subnet-save"
            className="text-ui h-7 rounded-pw bg-accent px-3 font-medium text-surface disabled:opacity-50"
            disabled={!canSave}
          >
            {state.mode === 'create' ? t('subnet.create') : t('subnet.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
