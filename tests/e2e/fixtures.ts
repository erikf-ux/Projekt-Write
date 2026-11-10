/**
 * Files for the media tests (Phase 8), made without further dependencies:
 * PNG (zlib of Node), ZIP with stored entries (for DOCX), PDF, the test PDF
 * of CVE-2024-4367 and an SVG with scripts. Written into the sandbox only.
 */
import zlib from 'node:zlib';

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) {
    c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  }
  return c >>> 0;
});

function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc = (CRC_TABLE[(crc ^ byte) & 0xff] ?? 0) ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Buffer): Buffer {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

/** A PNG of `width` × `height` pixels in stripes of `color` (RGB). */
export function png(width: number, height: number, color: [number, number, number]): Buffer {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 2; // RGB
  const raw = Buffer.alloc((width * 3 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const row = y * (width * 3 + 1);
    raw[row] = 0;
    for (let x = 0; x < width; x += 1) {
      const shade = (x + y) % 16 < 8 ? 1 : 0.7;
      raw[row + 1 + x * 3] = Math.round(color[0] * shade);
      raw[row + 2 + x * 3] = Math.round(color[1] * shade);
      raw[row + 3 + x * 3] = Math.round(color[2] * shade);
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', zlib.deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A ZIP whose entries are stored without compression. */
export function zip(entries: Record<string, string>): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, content] of Object.entries(entries)) {
    const nameBytes = Buffer.from(name, 'utf8');
    const data = Buffer.from(content, 'utf8');
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    locals.push(local, nameBytes, data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBytes);
    offset += 30 + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(entries).length, 8);
  end.writeUInt16LE(Object.keys(entries).length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

const DOCX_MAIN = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml';

/** A DOCX (main part only); `macro` makes it a macro-enabled one. */
export function docx(macro = false): Buffer {
  const type = macro ? 'application/vnd.ms-word.document.macroEnabled.main+xml' : DOCX_MAIN;
  return zip({
    '[Content_Types].xml': `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="${type}"/></Types>`,
    'word/document.xml':
      '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>',
  });
}

/** A PDF from its objects (1 0 obj is the catalog), with a correct xref table. */
function pdfFrom(objects: (string | Buffer)[]): Buffer {
  const parts: Buffer[] = [Buffer.from('%PDF-1.4\n%\xe2\xe3\xcf\xd3\n', 'latin1')];
  const offsets: number[] = [];
  let length = parts[0]?.length ?? 0;
  objects.forEach((object, index) => {
    offsets.push(length);
    const body = Buffer.concat([
      Buffer.from(`${String(index + 1)} 0 obj\n`, 'latin1'),
      typeof object === 'string' ? Buffer.from(object, 'latin1') : object,
      Buffer.from('\nendobj\n', 'latin1'),
    ]);
    parts.push(body);
    length += body.length;
  });
  const xref = [`xref\n0 ${String(objects.length + 1)}\n0000000000 65535 f \n`];
  for (const offset of offsets) {
    xref.push(`${String(offset).padStart(10, '0')} 00000 n \n`);
  }
  xref.push(
    `trailer\n<< /Size ${String(objects.length + 1)} /Root 1 0 R >>\nstartxref\n${String(length)}\n%%EOF\n`,
  );
  parts.push(Buffer.from(xref.join(''), 'latin1'));
  return Buffer.concat(parts);
}

function stream(content: string): string {
  return `<< /Length ${String(Buffer.byteLength(content, 'latin1'))} >>\nstream\n${content}\nendstream`;
}

/** A PDF of `pages` pages with text, a web link on page 1 and a link to the last page. */
export function pdf(pages: number, words: string): Buffer {
  const objects: string[] = ['<< /Type /Catalog /Pages 2 0 R >>'];
  const kids = Array.from({ length: pages }, (_, index) => `${String(4 + index * 2)} 0 R`).join(' ');
  objects.push(`<< /Type /Pages /Kids [${kids}] /Count ${String(pages)} >>`);
  objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  for (let page = 1; page <= pages; page += 1) {
    const pageObject = 4 + (page - 1) * 2;
    const annots =
      page === 1
        ? ` /Annots [<< /Type /Annot /Subtype /Link /Rect [72 640 300 660] /A << /S /URI /URI (https://example.org/quelle) >> >> << /Type /Annot /Subtype /Link /Rect [72 600 300 620] /Dest [${String(4 + (pages - 1) * 2)} 0 R /Fit] >>]`
        : '';
    objects.push(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents ${String(pageObject + 1)} 0 R /Resources << /Font << /F1 3 0 R >> >>${annots} >>`,
    );
    objects.push(
      stream(
        `BT /F1 20 Tf 72 700 Td (${words} ${String(page)}) Tj ET BT /F1 12 Tf 72 645 Td (Quelle im Netz) Tj ET BT /F1 12 Tf 72 605 Td (Zur letzten Seite) Tj ET`,
      ),
    );
  }
  return pdfFrom(objects);
}

/**
 * A test PDF after the pattern of CVE-2024-4367 (PDF.js before 4.2.67): a
 * Type1 font whose `FontMatrix` smuggles code into the glyph drawing code.
 * Vulnerable versions ran it with `new Function`; it sets a marker and the
 * window title. Nothing of it may run.
 */
export function cvePdf(): Buffer {
  const payload =
    "(1\\); window.__pwCve = 1; document.title = 'PWNED-PDF'; try { window.parent.__pwCve = 1 } catch (e) {} //)";
  const fontProgram = Buffer.from(
    '%!PS-AdobeFont-1.0: FooBarFont 001.000\n' +
      '/FontName /FooBarFont def\n/FontType 1 def\n/PaintType 0 def\n' +
      `/FontMatrix [0.001 0 0 0.001 0 ${payload}] readonly def\n` +
      '/FontBBox {0 0 1000 1000} readonly def\n/Encoding StandardEncoding def\ncurrentdict end\ncurrentfile eexec\n',
    'latin1',
  );
  const objects: (string | Buffer)[] = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
    stream('BT /F1 24 Tf 72 700 Td (CVE-2024-4367 Testdatei) Tj ET'),
    `<< /Type /Font /Subtype /Type1 /BaseFont /FooBarFont /FirstChar 32 /LastChar 126 /FontDescriptor 6 0 R /FontMatrix [0.1 0 0 0.1 0 ${payload}] >>`,
    '<< /Type /FontDescriptor /FontName /FooBarFont /Flags 32 /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile 7 0 R >>',
    Buffer.concat([
      Buffer.from(
        `<< /Length ${String(fontProgram.length)} /Length1 ${String(fontProgram.length)} /Length2 0 /Length3 0 >>\nstream\n`,
        'latin1',
      ),
      fontProgram,
      Buffer.from('\nendstream', 'latin1'),
    ]),
  ];
  return pdfFrom(objects);
}

/** An SVG that tries to run script in every way an SVG can. */
export const SCRIPT_SVG = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="120" height="80" onload="window.top.__pwSvg = 1; document.title = 'PWNED-SVG'">
  <script>window.top.__pwSvg = 2; parent.document.title = 'PWNED-SVG';</script>
  <rect width="120" height="80" fill="#3a7bd5"/>
  <foreignObject width="120" height="80">
    <body xmlns="http://www.w3.org/1999/xhtml"><script>window.top.__pwSvg = 3;</script><img src="x" onerror="window.top.__pwSvg = 4"/></body>
  </foreignObject>
  <a xlink:href="javascript:window.top.__pwSvg = 5"><text x="10" y="40">Klick</text></a>
</svg>
`;
