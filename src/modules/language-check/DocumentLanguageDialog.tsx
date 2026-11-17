/**
 * The language profile of a document (section 14, "Sprachprofile pro
 * Dokument"): its main language, further languages whose words count as
 * correct, and whether the autocorrect works in it.
 */
import { useEffect, useId, useState } from 'react';
import { create } from 'zustand';
import { Dialog } from '@/components/Dialog';
import { useT } from '@/i18n';
import { defaultLanguage, useLanguage } from './languageStore';

export const useProfileDialog = create<{ documentId: string | null }>(() => ({ documentId: null }));

export function openProfileDialog(documentId: string): void {
  void useLanguage.getState().loadDictionaries();
  void useLanguage
    .getState()
    .loadProfile(documentId)
    .then(() => {
      useProfileDialog.setState({ documentId });
    });
}

const SECONDARY = 'text-ui inline-flex h-7 items-center gap-2 rounded-pw border border-muted px-3 text-fg';
const PRIMARY =
  'text-ui inline-flex h-7 items-center gap-2 rounded-pw bg-accent px-3 font-medium text-surface';
const INPUT = 'text-ui h-7 rounded-pw border border-muted bg-surface px-2 text-fg';

export function DocumentLanguageDialog() {
  const documentId = useProfileDialog((state) => state.documentId);
  if (!documentId) {
    return null;
  }
  return <ProfileForm documentId={documentId} />;
}

function ProfileForm({ documentId }: { documentId: string }) {
  const t = useT();
  const dictionaries = useLanguage((state) => state.dictionaries);
  const stored = useLanguage((state) => state.profiles[documentId]);
  const [main, setMain] = useState(stored?.languages[0] ?? '');
  const [more, setMore] = useState<string[]>(stored?.languages.slice(1) ?? []);
  const [autocorrect, setAutocorrect] = useState(stored?.autocorrect ?? true);
  const [failed, setFailed] = useState(false);
  const id = useId();
  const close = () => {
    useProfileDialog.setState({ documentId: null });
  };

  useEffect(() => {
    void useLanguage.getState().loadDictionaries();
  }, []);

  const save = async () => {
    const languages = main === '' ? [] : [main, ...more.filter((tag) => tag !== main)];
    const done = await useLanguage.getState().setProfile(documentId, { languages, autocorrect });
    if (done) {
      close();
    } else {
      setFailed(true);
    }
  };

  return (
    <Dialog title={t('language.profile.title')} onClose={close} testId="language-profile">
      <label htmlFor={id} className="text-ui mr-2 text-fg">
        {t('language.profile.main')}
      </label>
      <select
        id={id}
        className={INPUT}
        data-testid="language-profile-main"
        value={main}
        onChange={(event) => {
          setMain(event.target.value);
        }}
      >
        <option value="">{t('language.profile.default', { language: defaultLanguage() })}</option>
        {dictionaries.map((dictionary) => (
          <option key={dictionary.tag} value={dictionary.tag}>
            {dictionary.tag}
          </option>
        ))}
      </select>
      <fieldset className="mt-3 border-0 p-0">
        <legend className="text-ui mb-1 text-fg">{t('language.profile.more')}</legend>
        {dictionaries
          .filter((dictionary) => dictionary.tag !== main)
          .map((dictionary) => (
            <label key={dictionary.tag} className="text-ui mr-3 inline-flex items-center gap-1 text-fg">
              <input
                type="checkbox"
                disabled={main === ''}
                checked={more.includes(dictionary.tag)}
                onChange={(event) => {
                  setMore(
                    event.target.checked
                      ? [...more, dictionary.tag]
                      : more.filter((tag) => tag !== dictionary.tag),
                  );
                }}
              />
              {dictionary.tag}
            </label>
          ))}
      </fieldset>
      <label className="text-ui mt-3 flex items-center gap-2 text-fg">
        <input
          type="checkbox"
          data-testid="language-profile-autocorrect"
          checked={autocorrect}
          onChange={(event) => {
            setAutocorrect(event.target.checked);
          }}
        />
        {t('language.profile.autocorrect')}
      </label>
      {failed ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {t('error.internal')}
        </p>
      ) : null}
      <div className="mt-4 flex justify-end gap-2">
        <button type="button" className={SECONDARY} onClick={close}>
          {t('language.profile.cancel')}
        </button>
        <button
          type="button"
          className={PRIMARY}
          data-testid="language-profile-save"
          onClick={() => void save()}
        >
          {t('language.profile.save')}
        </button>
      </div>
    </Dialog>
  );
}
