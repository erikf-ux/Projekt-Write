/**
 * Measurements of the end-to-end tests: printed where they are taken and
 * collected in a file, which `onComplete` (wdio.conf.ts) prints again at
 * the very end of a run, so a CI log shows them in its last lines.
 */
import fs from 'node:fs';
import path from 'node:path';
import { sandbox } from './sandbox';

export const measurementsFile = path.join(sandbox, 'measurements.txt');

export function measured(line: string): void {
  console.log(line);
  fs.appendFileSync(measurementsFile, `${line}\n`);
}
