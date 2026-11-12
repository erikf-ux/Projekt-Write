/**
 * Reads a PDF the app printed (export-backup.e2e.ts) with pdf.js, which the
 * app already uses for its PDF view: page sizes, text, link targets and
 * drawn images, page by page. Runs in Node, outside the app.
 */
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs';

export interface PdfPage {
  /** Width and height in points (1/72 inch). */
  width: number;
  height: number;
  /** The text of the page, pieces joined by spaces. */
  text: string;
  /** Smallest and largest x and y of a text piece, in points from the lower left. */
  textBox: { left: number; right: number; bottom: number; top: number } | null;
  /** Targets of the link areas. */
  links: string[];
  /** Images drawn on the page. */
  images: number;
}

export async function readPdf(bytes: Uint8Array): Promise<PdfPage[]> {
  const task = getDocument({ data: bytes });
  const pdf = await task.promise;
  const pages: PdfPage[] = [];
  for (let number = 1; number <= pdf.numPages; number += 1) {
    const page = await pdf.getPage(number);
    const [, , width = 0, height = 0] = page.view;
    const content = await page.getTextContent();
    const pieces = content.items.flatMap((item) =>
      'str' in item && item.str.trim() !== ''
        ? [{ text: item.str, x: Number(item.transform[4]), y: Number(item.transform[5]), w: item.width }]
        : [],
    );
    const textBox =
      pieces.length === 0
        ? null
        : {
            left: Math.min(...pieces.map((piece) => piece.x)),
            right: Math.max(...pieces.map((piece) => piece.x + piece.w)),
            bottom: Math.min(...pieces.map((piece) => piece.y)),
            top: Math.max(...pieces.map((piece) => piece.y)),
          };
    const annotations = (await page.getAnnotations()) as { subtype?: string; url?: string }[];
    const operators = await page.getOperatorList();
    pages.push({
      width,
      height,
      text: pieces
        .map((piece) => piece.text)
        .join(' ')
        .replace(/\s+/g, ' '),
      textBox,
      links: annotations.filter((note) => note.subtype === 'Link').map((note) => note.url ?? ''),
      images: operators.fnArray.filter((operator) => operator === OPS.paintImageXObject).length,
    });
  }
  await task.destroy();
  return pages;
}
