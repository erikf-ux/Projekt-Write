/**
 * Questions with a few answers (section 9: "Speichern, Verwerfen,
 * Abbrechen"; large files; duplicates). A flow awaits `ask`, the dialog
 * shows the question, the answer resolves the promise. Questions wait in
 * order; Escape gives the `cancel` answer.
 */
import { create } from 'zustand';
import type { MessageKey } from '@/i18n';

export interface PromptChoice<T extends string> {
  id: T;
  labelKey: MessageKey;
  tone?: 'primary' | 'danger' | 'default';
}

export interface PromptRequest<T extends string> {
  titleKey: MessageKey;
  textKey: MessageKey;
  params?: Record<string, string | number>;
  choices: readonly PromptChoice<T>[];
  /** The answer of Escape and the close button. */
  cancel: T;
  testId?: string;
}

interface PendingPrompt {
  request: PromptRequest<string>;
  resolve: (answer: string) => void;
}

interface PromptStore {
  queue: PendingPrompt[];
  answer: (answer: string) => void;
}

export const usePrompt = create<PromptStore>((set, get) => ({
  queue: [],
  answer: (answer) => {
    const [first, ...rest] = get().queue;
    if (!first) {
      return;
    }
    set({ queue: rest });
    first.resolve(answer);
  },
}));

/** Shows a question and waits for the answer. */
export function ask<T extends string>(request: PromptRequest<T>): Promise<T> {
  return new Promise<T>((resolve) => {
    usePrompt.setState((state) => ({
      queue: [
        ...state.queue,
        {
          request: request,
          resolve: (answer) => {
            resolve(answer as T);
          },
        },
      ],
    }));
  });
}
