import type { Locale, MessageKey } from '@/i18n';

const UNITS: { key: MessageKey; size: number }[] = [
  { key: 'units.gigabytes', size: 1024 ** 3 },
  { key: 'units.megabytes', size: 1024 ** 2 },
  { key: 'units.kilobytes', size: 1024 },
];

/** A byte count as message key plus localised number (1024-based units). */
export function formatBytes(bytes: number, locale: Locale): { key: MessageKey; value: string } {
  const unit = UNITS.find((candidate) => bytes >= candidate.size);
  if (!unit) {
    return { key: 'units.bytes', value: new Intl.NumberFormat(locale).format(bytes) };
  }
  const value = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / unit.size);
  return { key: unit.key, value };
}
