/**
 * Settings of the language check (section 14): what it does with and
 * without the add-on (shown openly), the switches, the default language,
 * language dictionaries, own dictionaries with their words and targets,
 * and the corrections the user took back.
 */
import { Download, Plus, Trash2, Upload } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { errorText } from '@/components/errorText';
import { useT, type MessageKey } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useUi } from '@/modules/settings/uiStore';
import { useStructure } from '@/modules/structure/structureStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { LanguageCheckSettings } from '@/types/generated/LanguageCheckSettings';
import type { OwnDictionary } from '@/types/generated/OwnDictionary';
import type { OwnWord } from '@/types/generated/OwnWord';
import type { Rejection } from '@/types/generated/Rejection';
import type { WordCategory } from '@/types/generated/WordCategory';
import { DEFAULT_CHECK_SETTINGS, useLanguage } from './languageStore';
import { useLanguageTool } from './languageToolStore';
import type { LanguageToolStatus } from '@/types/generated/LanguageToolStatus';

const SECONDARY =
  'text-ui inline-flex h-7 items-center gap-2 rounded-pw border border-muted px-3 text-fg disabled:opacity-60';
const FIELDSET = 'mb-5 border-0 p-0';
const LEGEND = 'text-ui mb-2 font-semibold tracking-wide text-muted uppercase';
const INPUT = 'text-ui h-7 rounded-pw border border-muted bg-surface px-2 text-fg';

/** Section 14, "Was welches Prüfwerk leistet". */
const CAPABILITIES: { key: MessageKey; without: boolean; with: MessageKey | null }[] = [
  { key: 'language.capability.spelling', without: true, with: null },
  { key: 'language.capability.rules', without: true, with: null },
  { key: 'language.capability.grammar', without: false, with: null },
  { key: 'language.capability.context', without: false, with: 'language.capability.contextWith' },
  { key: 'language.capability.meaning', without: false, with: 'language.capability.no' },
];

const CATEGORIES: WordCategory[] = ['word', 'term', 'name', 'abbreviation'];

export function LanguageSettings() {
  const t = useT();
  const state = useUi((store) => store.state);
  const update = useUi((store) => store.update);
  const dictionaries = useLanguage((store) => store.dictionaries);
  const [error, setError] = useState<CommandError | null>(null);
  const [tag, setTag] = useState('');
  const ids = { language: useId(), tag: useId() };

  useEffect(() => {
    void useLanguage.getState().loadDictionaries();
  }, []);

  if (!state) {
    return null;
  }
  const settings: LanguageCheckSettings = state.settings.languageCheck;
  const change = (patch: Partial<LanguageCheckSettings>) =>
    void update({ languageCheck: { ...DEFAULT_CHECK_SETTINGS, ...settings, ...patch } });

  const importLanguage = async () => {
    setError(null);
    const result = await call('language_dictionary_import', {
      tag: tag.trim(),
      title: t('language.dict.importTitle'),
    });
    if (result.ok) {
      setTag('');
      await useLanguage.getState().loadDictionaries();
      useLanguage.getState().bump();
    } else {
      setError(result.error);
    }
  };

  const removeLanguage = async (remove: string) => {
    setError(null);
    const result = await call('language_dictionary_remove', { tag: remove });
    if (result.ok) {
      await useLanguage.getState().loadDictionaries();
      useLanguage.getState().bump();
    } else {
      setError(result.error);
    }
  };

  const switches: { key: keyof LanguageCheckSettings; label: MessageKey }[] = [
    { key: 'enabled', label: 'language.settings.enabled' },
    { key: 'rules', label: 'language.settings.rules' },
    { key: 'autocorrect', label: 'language.settings.autocorrect' },
    { key: 'detect', label: 'language.settings.detect' },
  ];

  return (
    <div data-testid="language-settings">
      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('language.settings.check')}</legend>
        {switches.map((item) => (
          <label key={item.key} className="text-ui mb-1 flex items-center gap-2 text-fg">
            <input
              type="checkbox"
              data-testid={`language-${item.key}`}
              checked={settings[item.key] === true}
              disabled={item.key !== 'enabled' && !settings.enabled}
              onChange={(event) => {
                change({
                  [item.key]: event.target.checked,
                  ...(item.key === 'autocorrect' ? { autocorrectAsked: true } : {}),
                });
              }}
            />
            {t(item.label)}
          </label>
        ))}
        <label htmlFor={ids.language} className="text-ui mt-2 mr-2 text-fg">
          {t('language.settings.default')}
        </label>
        <select
          id={ids.language}
          data-testid="language-default"
          className={INPUT}
          value={settings.defaultLanguage ?? ''}
          onChange={(event) => {
            change({ defaultLanguage: event.target.value === '' ? null : event.target.value });
          }}
        >
          <option value="">{t('language.settings.fromInterface')}</option>
          {dictionaries.map((dictionary) => (
            <option key={dictionary.tag} value={dictionary.tag}>
              {dictionary.tag}
            </option>
          ))}
        </select>
      </fieldset>

      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('language.capability.title')}</legend>
        <table className="text-ui w-full text-left text-fg" data-testid="language-capabilities">
          <thead>
            <tr className="text-muted">
              <th className="py-1 font-normal">{t('language.capability.check')}</th>
              <th className="py-1 font-normal">{t('language.capability.without')}</th>
              <th className="py-1 font-normal">{t('language.capability.with')}</th>
            </tr>
          </thead>
          <tbody>
            {CAPABILITIES.map((row) => (
              <tr key={row.key}>
                <td className="py-0.5 pr-2">{t(row.key)}</td>
                <td className="py-0.5 pr-2">
                  {t(row.without ? 'language.capability.yes' : 'language.capability.no')}
                </td>
                <td className="py-0.5">{t(row.with ?? 'language.capability.yes')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </fieldset>

      <fieldset className={FIELDSET}>
        <legend className={LEGEND}>{t('language.dict.languages')}</legend>
        <ul className="text-ui mb-2 text-fg" data-testid="language-dictionaries">
          {dictionaries.map((dictionary) => (
            <li key={dictionary.tag} className="flex items-center gap-2 py-0.5" data-tag={dictionary.tag}>
              <span className="min-w-20 font-medium">{dictionary.tag}</span>
              <span className="text-muted">
                {t(dictionary.source === 'bundled' ? 'language.dict.bundled' : 'language.dict.added')}
              </span>
              {dictionary.source === 'added' ? (
                <button
                  type="button"
                  className={SECONDARY}
                  aria-label={t('language.dict.remove', { tag: dictionary.tag })}
                  onClick={() => void removeLanguage(dictionary.tag)}
                >
                  <Trash2 aria-hidden="true" className="size-3.5" />
                </button>
              ) : null}
            </li>
          ))}
        </ul>
        <div className="flex items-center gap-2">
          <label htmlFor={ids.tag} className="text-ui text-fg">
            {t('language.dict.tag')}
          </label>
          <input
            id={ids.tag}
            className={`${INPUT} w-28`}
            placeholder="fr-FR"
            value={tag}
            onChange={(event) => {
              setTag(event.target.value);
            }}
          />
          <button
            type="button"
            className={SECONDARY}
            disabled={tag.trim() === ''}
            onClick={() => void importLanguage()}
          >
            <Upload aria-hidden="true" className="size-3.5" />
            {t('language.dict.import')}
          </button>
        </div>
        {error ? (
          <p role="alert" className="text-ui mt-2 text-danger">
            {errorText(t, error)}
          </p>
        ) : null}
      </fieldset>

      <LanguageToolSection />
      <OwnDictionaries />
      <Rejections />
    </div>
  );
}

/** LanguageTool as an add-on: an existing installation, nothing downloaded. */
function LanguageToolSection() {
  const t = useT();
  const status = useLanguageTool((store) => store.status);
  const [error, setError] = useState<CommandError | null>(null);

  useEffect(() => {
    void useLanguageTool.getState().refresh();
  }, []);

  const run = async (
    action: Promise<{ ok: true; value: LanguageToolStatus | null } | { ok: false; error: CommandError }>,
  ) => {
    setError(null);
    const result = await action;
    if (!result.ok) {
      setError(result.error);
    } else if (result.value) {
      useLanguageTool.getState().apply(result.value);
    }
  };

  const stateText = status
    ? !status.configured
      ? t('language.tool.none')
      : !status.enabled
        ? t('language.tool.off')
        : status.state === 'failed'
          ? t(status.problem === 'java' ? 'language.tool.noJava' : 'language.tool.failed')
          : t(`language.tool.${status.state}` as MessageKey)
    : '';

  return (
    <fieldset className={FIELDSET}>
      <legend className={LEGEND}>{t('language.tool.title')}</legend>
      <p className="text-ui mb-2 text-muted">{t('language.tool.about')}</p>
      <p
        className="text-ui mb-2 text-fg"
        role="status"
        data-testid="languagetool-state"
        data-state={status?.state}
      >
        {stateText}
        {status?.folder ? <span className="ml-2 text-muted">{status.folder}</span> : null}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          className={SECONDARY}
          data-testid="languagetool-choose"
          onClick={() => void run(call('languagetool_choose', { title: t('language.tool.chooseTitle') }))}
        >
          {t('language.tool.choose')}
        </button>
        {status?.configured ? (
          <>
            <label className="text-ui flex items-center gap-2 text-fg">
              <input
                type="checkbox"
                data-testid="languagetool-enabled"
                checked={status.enabled}
                onChange={(event) =>
                  void run(call('languagetool_set_enabled', { enabled: event.target.checked }))
                }
              />
              {t('language.tool.enabled')}
            </label>
            <button
              type="button"
              className={SECONDARY}
              onClick={() => void run(call('languagetool_forget', {}))}
            >
              {t('language.tool.forget')}
            </button>
          </>
        ) : null}
      </div>
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </fieldset>
  );
}

function OwnDictionaries() {
  const t = useT();
  const [list, setList] = useState<OwnDictionary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [error, setError] = useState<CommandError | null>(null);
  const id = useId();

  const load = async () => {
    const result = await call('dictionaries_list', {});
    if (result.ok) {
      setList(result.value);
    }
  };

  useEffect(() => {
    let current = true;
    void call('dictionaries_list', {}).then((result) => {
      if (current && result.ok) {
        setList(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, []);

  const create = async () => {
    setError(null);
    const result = await call('dictionary_create', { name: name.trim(), forAll: true });
    if (result.ok) {
      setName('');
      setSelected(result.value.id);
      await load();
      useLanguage.getState().bump();
    } else {
      setError(result.error);
    }
  };

  const current = list.find((dictionary) => dictionary.id === selected) ?? null;

  return (
    <fieldset className={FIELDSET}>
      <legend className={LEGEND}>{t('language.own.title')}</legend>
      <ul className="text-ui mb-2 text-fg" data-testid="own-dictionaries">
        {list.map((dictionary) => (
          <li key={dictionary.id}>
            <button
              type="button"
              className={`w-full rounded-pw px-2 py-0.5 text-left ${dictionary.id === selected ? 'bg-accent text-surface' : ''}`}
              aria-pressed={dictionary.id === selected}
              data-dictionary={dictionary.name}
              onClick={() => {
                setSelected(dictionary.id === selected ? null : dictionary.id);
              }}
            >
              {dictionary.name} · {t('language.own.words', { count: dictionary.words })} ·{' '}
              {dictionary.forAll
                ? t('language.own.forAll')
                : t('language.own.targets', {
                    documents: dictionary.documents.length,
                    subnets: dictionary.subnets.length,
                  })}
            </button>
          </li>
        ))}
      </ul>
      <div className="flex items-center gap-2">
        <label htmlFor={id} className="text-ui text-fg">
          {t('language.own.name')}
        </label>
        <input
          id={id}
          className={INPUT}
          value={name}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <button
          type="button"
          className={SECONDARY}
          disabled={name.trim() === ''}
          onClick={() => void create()}
        >
          <Plus aria-hidden="true" className="size-3.5" />
          {t('language.own.create')}
        </button>
      </div>
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
      {current ? (
        <DictionaryEditor
          key={current.id}
          dictionary={current}
          onChanged={() => {
            void load();
            useLanguage.getState().bump();
          }}
          onDeleted={() => {
            setSelected(null);
            void load();
            useLanguage.getState().bump();
          }}
        />
      ) : null}
    </fieldset>
  );
}

function DictionaryEditor({
  dictionary,
  onChanged,
  onDeleted,
}: {
  dictionary: OwnDictionary;
  onChanged: () => void;
  onDeleted: () => void;
}) {
  const t = useT();
  const [words, setWords] = useState<OwnWord[]>([]);
  const [word, setWord] = useState('');
  const [category, setCategory] = useState<WordCategory>('word');
  const [rename, setRename] = useState(dictionary.name);
  const [error, setError] = useState<CommandError | null>(null);
  const [report, setReport] = useState<string | null>(null);
  const [filter, setFilter] = useState('');
  const subnets = useStructure((store) => store.subnets);
  const documents = useWorkspace((store) => store.documents);
  const ids = { word: useId(), category: useId(), rename: useId(), filter: useId() };

  const loadWords = async () => {
    const result = await call('dictionary_words', { id: dictionary.id });
    if (result.ok) {
      setWords(result.value);
    }
  };

  useEffect(() => {
    let current = true;
    void call('dictionary_words', { id: dictionary.id }).then((result) => {
      if (current && result.ok) {
        setWords(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, [dictionary.id]);

  const run = async (action: () => Promise<{ ok: boolean; error?: CommandError }>) => {
    setError(null);
    const result = await action();
    if (!result.ok && result.error) {
      setError(result.error);
      return false;
    }
    await loadWords();
    onChanged();
    return true;
  };

  const assign = (patch: { forAll?: boolean; documents?: string[]; subnets?: string[] }) =>
    run(() =>
      call('dictionary_assign', {
        id: dictionary.id,
        forAll: patch.forAll ?? dictionary.forAll,
        documents: patch.documents ?? dictionary.documents,
        subnets: patch.subnets ?? dictionary.subnets,
      }),
    );

  const toggle = (values: readonly string[], value: string) =>
    values.includes(value) ? values.filter((v) => v !== value) : [...values, value];

  const visibleDocuments = documents.filter(
    (document) => filter.trim() === '' || document.title.toLowerCase().includes(filter.trim().toLowerCase()),
  );

  return (
    <div className="mt-3 rounded-pw border border-muted p-3" data-testid="dictionary-editor">
      <div className="mb-2 flex flex-wrap items-center gap-2">
        <label htmlFor={ids.rename} className="text-ui text-fg">
          {t('language.own.name')}
        </label>
        <input
          id={ids.rename}
          className={INPUT}
          value={rename}
          onChange={(event) => {
            setRename(event.target.value);
          }}
        />
        <button
          type="button"
          className={SECONDARY}
          disabled={rename.trim() === '' || rename === dictionary.name}
          onClick={() =>
            void run(() => call('dictionary_rename', { id: dictionary.id, name: rename.trim() }))
          }
        >
          {t('language.own.rename')}
        </button>
        <button
          type="button"
          className={SECONDARY}
          onClick={() =>
            void call('dictionary_import', { id: dictionary.id, title: t('language.own.importTitle') }).then(
              async (result) => {
                if (!result.ok) {
                  setError(result.error);
                  return;
                }
                if (result.value) {
                  setReport(
                    t('language.own.imported', {
                      added: result.value.added,
                      known: result.value.known,
                      skipped: result.value.skipped,
                    }),
                  );
                  await loadWords();
                  onChanged();
                }
              },
            )
          }
        >
          <Upload aria-hidden="true" className="size-3.5" />
          {t('language.own.import')}
        </button>
        <button
          type="button"
          className={SECONDARY}
          onClick={() =>
            void call('dictionary_export', { id: dictionary.id, title: t('language.own.exportTitle') }).then(
              (result) => {
                if (!result.ok) {
                  setError(result.error);
                }
              },
            )
          }
        >
          <Download aria-hidden="true" className="size-3.5" />
          {t('language.own.export')}
        </button>
        <button
          type="button"
          className={SECONDARY}
          onClick={() =>
            void call('dictionary_delete', { id: dictionary.id }).then((result) => {
              if (result.ok) {
                onDeleted();
              } else {
                setError(result.error);
              }
            })
          }
        >
          <Trash2 aria-hidden="true" className="size-3.5" />
          {t('language.own.delete')}
        </button>
      </div>
      {report ? (
        <p role="status" className="text-ui mb-2 text-muted">
          {report}
        </p>
      ) : null}

      <div className="mb-2 flex flex-wrap items-center gap-2">
        <label htmlFor={ids.word} className="text-ui text-fg">
          {t('language.own.word')}
        </label>
        <input
          id={ids.word}
          className={INPUT}
          data-testid="own-word-input"
          value={word}
          onChange={(event) => {
            setWord(event.target.value);
          }}
        />
        <label htmlFor={ids.category} className="sr-only">
          {t('language.own.category')}
        </label>
        <select
          id={ids.category}
          className={INPUT}
          value={category}
          onChange={(event) => {
            setCategory(event.target.value as WordCategory);
          }}
        >
          {CATEGORIES.map((value) => (
            <option key={value} value={value}>
              {t(`language.category.${value}` as MessageKey)}
            </option>
          ))}
        </select>
        <button
          type="button"
          className={SECONDARY}
          data-testid="own-word-add"
          disabled={word.trim() === ''}
          onClick={() =>
            void run(() =>
              call('dictionary_word_add', { id: dictionary.id, word: word.trim(), category }),
            ).then((done) => {
              if (done) {
                setWord('');
              }
            })
          }
        >
          <Plus aria-hidden="true" className="size-3.5" />
          {t('language.own.add')}
        </button>
      </div>
      <ul className="text-ui mb-3 max-h-40 overflow-auto text-fg" data-testid="own-words">
        {words.map((entry) => (
          <li key={entry.word} className="flex items-center gap-2 py-0.5" data-word={entry.word}>
            <span className="min-w-32">{entry.word}</span>
            <select
              aria-label={t('language.own.categoryOf', { word: entry.word })}
              className={INPUT}
              value={entry.category}
              onChange={(event) =>
                void run(() =>
                  call('dictionary_word_update', {
                    id: dictionary.id,
                    old: entry.word,
                    word: entry.word,
                    category: event.target.value as WordCategory,
                  }),
                )
              }
            >
              {CATEGORIES.map((value) => (
                <option key={value} value={value}>
                  {t(`language.category.${value}` as MessageKey)}
                </option>
              ))}
            </select>
            <button
              type="button"
              className={SECONDARY}
              aria-label={t('language.own.removeWord', { word: entry.word })}
              onClick={() =>
                void run(() => call('dictionary_word_remove', { id: dictionary.id, word: entry.word }))
              }
            >
              <Trash2 aria-hidden="true" className="size-3.5" />
            </button>
          </li>
        ))}
      </ul>

      <p className="text-ui mb-1 font-medium text-fg">{t('language.own.assign')}</p>
      <label className="text-ui mb-1 flex items-center gap-2 text-fg">
        <input
          type="checkbox"
          data-testid="own-for-all"
          checked={dictionary.forAll}
          onChange={(event) => void assign({ forAll: event.target.checked })}
        />
        {t('language.own.forAll')}
      </label>
      {subnets.length > 0 ? (
        <fieldset className="mb-2 border-0 p-0">
          <legend className="text-ui text-muted">{t('language.own.subnets')}</legend>
          {subnets.map((subnet) => (
            <label key={subnet.id} className="text-ui mr-3 inline-flex items-center gap-1 text-fg">
              <input
                type="checkbox"
                checked={dictionary.subnets.includes(subnet.id)}
                onChange={() => void assign({ subnets: toggle(dictionary.subnets, subnet.id) })}
              />
              {subnet.name}
            </label>
          ))}
        </fieldset>
      ) : null}
      <fieldset className="border-0 p-0">
        <legend className="text-ui text-muted">{t('language.own.documents')}</legend>
        <label htmlFor={ids.filter} className="sr-only">
          {t('language.own.filter')}
        </label>
        <input
          id={ids.filter}
          className={`${INPUT} mb-1`}
          placeholder={t('language.own.filter')}
          value={filter}
          onChange={(event) => {
            setFilter(event.target.value);
          }}
        />
        <div className="max-h-32 overflow-auto">
          {visibleDocuments.slice(0, 200).map((document) => (
            <label key={document.id} className="text-ui flex items-center gap-1 text-fg">
              <input
                type="checkbox"
                checked={dictionary.documents.includes(document.id)}
                onChange={() => void assign({ documents: toggle(dictionary.documents, document.id) })}
              />
              {document.title}
            </label>
          ))}
        </div>
      </fieldset>
      {error ? (
        <p role="alert" className="text-ui mt-2 text-danger">
          {errorText(t, error)}
        </p>
      ) : null}
    </div>
  );
}

function Rejections() {
  const t = useT();
  const [list, setList] = useState<Rejection[]>([]);

  const load = async () => {
    const result = await call('autocorrect_rejections', {});
    if (result.ok) {
      setList(result.value);
    }
  };

  useEffect(() => {
    let current = true;
    void call('autocorrect_rejections', {}).then((result) => {
      if (current && result.ok) {
        setList(result.value);
      }
    });
    return () => {
      current = false;
    };
  }, []);

  if (list.length === 0) {
    return null;
  }
  return (
    <fieldset className={FIELDSET}>
      <legend className={LEGEND}>{t('language.rejected.title')}</legend>
      <ul className="text-ui text-fg" data-testid="rejections">
        {list.map((entry) => (
          <li key={`${entry.word}\u0000${entry.correction}`} className="flex items-center gap-2 py-0.5">
            <span>
              {entry.word} → {entry.correction}
            </span>
            <button
              type="button"
              className={SECONDARY}
              onClick={() =>
                void call('autocorrect_forget', entry).then(async (result) => {
                  if (result.ok) {
                    await load();
                  }
                })
              }
            >
              {t('language.rejected.allow')}
            </button>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}
