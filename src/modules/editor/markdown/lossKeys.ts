/** Texts of the losses Markdown would cause (section 10, ADR 0014). */
import type { MessageKey } from '@/i18n';
import type { Loss } from './losses';

export const LOSS_KEYS: Record<Loss['kind'], MessageKey> = {
  tableMergedCells: 'editor.loss.tableMergedCells',
  tableCellBlocks: 'editor.loss.tableCellBlocks',
  tableHeader: 'editor.loss.tableHeader',
  tableAlignment: 'editor.loss.tableAlignment',
  tableColumnWidth: 'editor.loss.tableColumnWidth',
  listNumbering: 'editor.loss.listNumbering',
  emphasisBoundary: 'editor.loss.emphasisBoundary',
  codeBackticks: 'editor.loss.codeBackticks',
  htmlLineStart: 'editor.loss.htmlLineStart',
  changed: 'editor.loss.changed',
};
