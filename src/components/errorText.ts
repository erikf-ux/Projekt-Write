import { isMessageKey, type MessageKey } from '@/i18n';
import type { CommandError } from '@/types/generated/CommandError';

type Translate = (key: MessageKey, params?: Record<string, string | number>) => string;

/** The user-facing text of a command error, with its details as parameters. */
export function errorText(t: Translate, error: CommandError): string {
  const key = isMessageKey(error.messageKey) ? error.messageKey : 'error.internal';
  const params: Record<string, string> = {};
  for (const [name, value] of Object.entries(error.details ?? {})) {
    if (typeof value === 'string') {
      params[name] = value;
    }
  }
  return t(key, params);
}
