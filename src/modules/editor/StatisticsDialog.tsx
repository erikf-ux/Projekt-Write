/**
 * The writing statistics of the active document (section 12): words,
 * characters, characters without spaces, paragraphs and, if switched on,
 * the reading time; next to it the values of the selection.
 */
import { Dialog } from '@/components/Dialog';
import { useLocale, useT, type MessageKey } from '@/i18n';
import { readingMinutes, type TextStatistics } from './statistics';
import type { DocumentStatistics } from './statisticsStore';

interface StatisticsDialogProps {
  statistics: DocumentStatistics;
  /** Words per minute, or null if the reading time is switched off. */
  wordsPerMinute: number | null;
  onClose: () => void;
}

const ROWS: { key: keyof TextStatistics; label: MessageKey }[] = [
  { key: 'words', label: 'statistics.words' },
  { key: 'characters', label: 'statistics.characters' },
  { key: 'charactersWithoutSpaces', label: 'statistics.charactersWithoutSpaces' },
  { key: 'paragraphs', label: 'statistics.paragraphs' },
];

export function StatisticsDialog({ statistics, wordsPerMinute, onClose }: StatisticsDialogProps) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const number = new Intl.NumberFormat(locale);
  const columns: { label: MessageKey; values: TextStatistics }[] = [
    ...(statistics.selection
      ? [{ label: 'statistics.selection' as const, values: statistics.selection }]
      : []),
    { label: 'statistics.document', values: statistics.document },
  ];
  const minutes = (values: TextStatistics) =>
    wordsPerMinute === null
      ? ''
      : t('statistics.minutes', { count: number.format(readingMinutes(values.words, wordsPerMinute)) });
  return (
    <Dialog
      title={t('statistics.title')}
      onClose={onClose}
      testId="statistics-dialog"
      width="w-[min(420px,92vw)]"
    >
      <table className="text-ui w-full border-collapse" data-testid="statistics-table">
        <thead>
          <tr>
            <td />
            {columns.map((column) => (
              <th key={column.label} scope="col" className="px-2 py-1 text-right font-medium">
                {t(column.label)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {ROWS.map((row) => (
            <tr key={row.key} className="border-t border-muted/30">
              <th scope="row" className="py-1 pr-2 text-left font-normal text-muted">
                {t(row.label)}
              </th>
              {columns.map((column) => (
                <td key={column.label} className="px-2 py-1 text-right tabular-nums" data-statistic={row.key}>
                  {number.format(column.values[row.key])}
                </td>
              ))}
            </tr>
          ))}
          {wordsPerMinute !== null ? (
            <tr className="border-t border-muted/30">
              <th scope="row" className="py-1 pr-2 text-left font-normal text-muted">
                {t('statistics.readingTime', { speed: number.format(wordsPerMinute) })}
              </th>
              {columns.map((column) => (
                <td
                  key={column.label}
                  className="px-2 py-1 text-right tabular-nums"
                  data-statistic="readingTime"
                >
                  {minutes(column.values)}
                </td>
              ))}
            </tr>
          ) : null}
        </tbody>
      </table>
    </Dialog>
  );
}
