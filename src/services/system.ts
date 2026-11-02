import type { SystemCheckId } from '@/types/generated/SystemCheckId';
import { call } from './ipc';

/** All checks, in display order. */
export const ALL_CHECKS: readonly SystemCheckId[] = [
  'database',
  'migrations',
  'foreignKeys',
  'fullTextSearch',
];

export function runSystemCheck(checks: readonly SystemCheckId[] = ALL_CHECKS) {
  return call('system_check', { checks: [...checks] });
}
