/**
 * The question after a change from outside (ADR 0015, decision 7): an
 * external file was changed or removed while its tab is open. Closing the
 * dialog postpones the decision; a note above the text reopens it, and the
 * tab is not saved until the user decides.
 */
import { Dialog } from '@/components/Dialog';
import { translate, useLocale, useT } from '@/i18n';
import { useWorkspace, type OpenDocument } from './workspaceStore';

interface OutsideChangeDialogProps {
  document: OpenDocument;
}

const BUTTON = 'text-ui inline-flex h-7 items-center rounded-pw px-3';

export function OutsideChangeDialog({ document }: OutsideChangeDialogProps) {
  const t = useT();
  const external = document.external;
  if (!external?.outside || external.outsideDeferred) {
    return null;
  }
  const { id } = document;
  const workspace = useWorkspace.getState;
  const dialogTitle = (key: 'external.dialog.saveCopy' | 'external.dialog.saveAs') =>
    translate(useLocale.getState().locale, key);
  const changed = external.outside === 'changed';
  return (
    <Dialog
      title={t(changed ? 'external.changed.title' : 'external.missing.title')}
      onClose={() => {
        workspace().deferOutside(id, true);
      }}
      testId="outside-change"
      width="w-[min(520px,92vw)]"
    >
      <p className="text-ui">
        {t(changed ? 'external.changed.text' : 'external.missing.text', { name: external.name })}
      </p>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        {changed ? (
          <>
            <button
              type="button"
              data-autofocus
              data-choice="reload"
              className={`${BUTTON} bg-accent font-medium text-surface`}
              onClick={() => void workspace().reloadExternal(id)}
            >
              {t('external.changed.reload')}
            </button>
            <button
              type="button"
              data-choice="keepMine"
              className={`${BUTTON} border border-muted`}
              onClick={() => void workspace().keepMine(id)}
            >
              {t('external.changed.keepMine')}
            </button>
            <button
              type="button"
              data-choice="saveCopy"
              className={`${BUTTON} border border-muted`}
              onClick={() => void workspace().saveAs(id, dialogTitle('external.dialog.saveCopy'))}
            >
              {t('external.changed.saveCopy')}
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              data-autofocus
              data-choice="saveAs"
              className={`${BUTTON} bg-accent font-medium text-surface`}
              onClick={() => void workspace().saveAs(id, dialogTitle('external.dialog.saveAs'))}
            >
              {t('external.missing.saveAs')}
            </button>
            <button
              type="button"
              data-choice="keep"
              className={`${BUTTON} border border-muted`}
              onClick={() => void workspace().keepMine(id)}
            >
              {t('external.missing.keep')}
            </button>
            <button
              type="button"
              data-choice="close"
              className={`${BUTTON} border border-danger text-danger`}
              onClick={() => void workspace().closeTab(id)}
            >
              {t('external.missing.close')}
            </button>
          </>
        )}
      </div>
    </Dialog>
  );
}
