/**
 * Minimal PNG reader for E2E screenshots: 8-bit RGB or RGBA, not interlaced.
 * Uses only node:zlib, so the tests need no image library.
 */
import { inflateSync } from 'node:zlib';

export interface Image {
  width: number;
  height: number;
  /** RGBA, 4 bytes per pixel. */
  pixels: Uint8Array;
}

const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) {
    return a;
  }
  return pb <= pc ? b : c;
}

export function decodePng(data: Buffer): Image {
  if (!SIGNATURE.every((byte, index) => data[index] === byte)) {
    throw new Error('not a PNG file');
  }
  let offset = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const idat: Buffer[] = [];
  while (offset < data.length) {
    const length = data.readUInt32BE(offset);
    const type = data.toString('ascii', offset + 4, offset + 8);
    const chunk = data.subarray(offset + 8, offset + 8 + length);
    if (type === 'IHDR') {
      width = chunk.readUInt32BE(0);
      height = chunk.readUInt32BE(4);
      const bitDepth = chunk[8];
      const colorType = chunk[9];
      const interlace = chunk[12];
      if (bitDepth !== 8 || interlace !== 0 || (colorType !== 2 && colorType !== 6)) {
        throw new Error(`unsupported PNG (depth ${String(bitDepth)}, colour type ${String(colorType)})`);
      }
      channels = colorType === 6 ? 4 : 3;
    } else if (type === 'IDAT') {
      idat.push(chunk);
    } else if (type === 'IEND') {
      break;
    }
    offset += 12 + length;
  }

  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const current = new Uint8Array(stride);
  const previous = new Uint8Array(stride);
  const pixels = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const rowStart = y * (stride + 1);
    const filter = raw[rowStart] ?? 0;
    for (let x = 0; x < stride; x++) {
      const value = raw[rowStart + 1 + x] ?? 0;
      const left = x >= channels ? (current[x - channels] ?? 0) : 0;
      const up = previous[x] ?? 0;
      const upLeft = x >= channels ? (previous[x - channels] ?? 0) : 0;
      let decoded: number;
      switch (filter) {
        case 0:
          decoded = value;
          break;
        case 1:
          decoded = value + left;
          break;
        case 2:
          decoded = value + up;
          break;
        case 3:
          decoded = value + Math.floor((left + up) / 2);
          break;
        case 4:
          decoded = value + paeth(left, up, upLeft);
          break;
        default:
          throw new Error(`unknown PNG filter ${String(filter)}`);
      }
      current[x] = decoded & 0xff;
    }
    for (let x = 0; x < width; x++) {
      const target = (y * width + x) * 4;
      pixels[target] = current[x * channels] ?? 0;
      pixels[target + 1] = current[x * channels + 1] ?? 0;
      pixels[target + 2] = current[x * channels + 2] ?? 0;
      pixels[target + 3] = channels === 4 ? (current[x * channels + 3] ?? 255) : 255;
    }
    previous.set(current);
  }
  return { width, height, pixels };
}

/** `#RRGGBB` of the pixel at (x, y). */
export function pixelHex(image: Image, x: number, y: number): string {
  const index =
    (Math.min(image.height - 1, Math.max(0, y)) * image.width + Math.min(image.width - 1, Math.max(0, x))) *
    4;
  return `#${[0, 1, 2].map((offset) => (image.pixels[index + offset] ?? 0).toString(16).padStart(2, '0')).join('')}`.toUpperCase();
}
