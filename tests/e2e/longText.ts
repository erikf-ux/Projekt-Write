/**
 * A long German Markdown manuscript for the performance targets of section 6
 * ("Dokument mit 50.000 Wörtern öffnen", "Tippverzögerung bei 50.000
 * Wörtern"). The same seed gives the same text on every system.
 *
 * Every word of the vocabulary is one word segment of UAX #29, as the
 * writing statistics count them, so the generator knows the exact count.
 */

const NOUNS = [
  'Abend',
  'Antwort',
  'Ärger',
  'Bahnhof',
  'Baum',
  'Brief',
  'Brücke',
  'Buch',
  'Dach',
  'Dorf',
  'Erinnerung',
  'Fenster',
  'Feld',
  'Frage',
  'Freundin',
  'Garten',
  'Gedanke',
  'Geschichte',
  'Glück',
  'Größe',
  'Hafen',
  'Hand',
  'Haus',
  'Himmel',
  'Insel',
  'Jahr',
  'Kälte',
  'Kopf',
  'Küche',
  'Lampe',
  'Licht',
  'Meer',
  'Mond',
  'Morgen',
  'Mühe',
  'Nachbar',
  'Nacht',
  'Öffnung',
  'Regen',
  'Schiff',
  'Schlüssel',
  'Sonne',
  'Spiegel',
  'Stadt',
  'Stimme',
  'Straße',
  'Stuhl',
  'Stunde',
  'Tisch',
  'Tür',
  'Übung',
  'Ufer',
  'Wald',
  'Wärme',
  'Wasser',
  'Weg',
  'Woche',
  'Wolke',
  'Zeit',
  'Zimmer',
  'Zug',
];
const VERBS = [
  'antwortet',
  'bleibt',
  'bringt',
  'denkt',
  'erinnert',
  'erzählt',
  'fährt',
  'fällt',
  'findet',
  'fragt',
  'geht',
  'hält',
  'hört',
  'kommt',
  'lächelt',
  'läuft',
  'liest',
  'nimmt',
  'öffnet',
  'schläft',
  'schließt',
  'schreibt',
  'sieht',
  'sucht',
  'trägt',
  'träumt',
  'vergisst',
  'versteht',
  'wächst',
  'wartet',
];
const WORDS = [
  'aber',
  'alt',
  'am',
  'auch',
  'bei',
  'blau',
  'dann',
  'das',
  'der',
  'die',
  'doch',
  'dort',
  'dunkel',
  'durch',
  'ein',
  'eine',
  'fast',
  'früh',
  'für',
  'ganz',
  'gegen',
  'gestern',
  'grau',
  'groß',
  'grün',
  'heute',
  'hell',
  'hier',
  'hinter',
  'im',
  'immer',
  'kalt',
  'klein',
  'langsam',
  'leicht',
  'leise',
  'mit',
  'müde',
  'nach',
  'nah',
  'neben',
  'neu',
  'nicht',
  'noch',
  'nur',
  'oder',
  'ohne',
  'plötzlich',
  'rot',
  'schon',
  'schön',
  'schwer',
  'sehr',
  'spät',
  'später',
  'still',
  'über',
  'und',
  'unter',
  'vom',
  'vor',
  'warm',
  'weit',
  'weiß',
  'wieder',
  'zum',
  'zur',
];

/** Last paragraph; a page can tell from it that the whole text is shown. */
export const END_MARKER = 'Ende des Leistungstests';

/** Small seeded generator (mulberry32). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  };
}

export interface LongText {
  markdown: string;
  /** Words as the writing statistics count them. */
  words: number;
}

/**
 * About 300 KB of Markdown with exactly `target` words: chapters with
 * headings, paragraphs with bold and italic words, now and then a bullet
 * list or a quotation.
 */
export function longGermanText(target: number, seed = 20_261_104): LongText {
  const next = random(seed);
  const pick = <T>(list: readonly T[]): T => list[Math.floor(next() * list.length)] as T;
  const between = (min: number, max: number) => min + Math.floor(next() * (max - min + 1));
  const blocks: string[] = ['# Leistungstest'];
  // The title has one word, the end marker three.
  let words = 1;
  const budget = target - 3;

  const sentence = (length: number, end: string): string => {
    const parts: string[] = [];
    for (let index = 0; index < length; index += 1) {
      let word = index % 3 === 1 ? pick(VERBS) : index % 3 === 0 ? pick(NOUNS) : pick(WORDS);
      if (index === 0) {
        word = word.charAt(0).toUpperCase() + word.slice(1);
      }
      const style = next();
      if (style < 0.02) {
        word = `**${word}**`;
      } else if (style < 0.04) {
        word = `*${word}*`;
      }
      if (index < length - 1 && next() < 0.07) {
        word += ',';
      }
      parts.push(word);
    }
    return parts.join(' ') + end;
  };
  /** As many words as asked, or what is left of the budget. */
  const take = (wanted: number) => {
    const length = Math.min(wanted, budget - words);
    words += length;
    return length;
  };

  let chapter = 1;
  let untilHeading = 0;
  while (words < budget) {
    if (untilHeading === 0 && budget - words > 2) {
      words += 2;
      blocks.push(`## Kapitel ${String(chapter)}`);
      chapter += 1;
      untilHeading = between(12, 20);
    }
    untilHeading = Math.max(0, untilHeading - 1);
    const kind = next();
    if (kind < 0.04) {
      const items: string[] = [];
      for (let count = between(3, 5); count > 0 && words < budget; count -= 1) {
        items.push(`- ${sentence(take(between(3, 8)), '')}`);
      }
      blocks.push(items.join('\n'));
    } else if (kind < 0.07) {
      blocks.push(`> ${sentence(take(between(8, 16)), '.')}`);
    } else {
      const sentences: string[] = [];
      for (let count = between(3, 7); count > 0 && words < budget; count -= 1) {
        const end = pick(['.', '.', '.', '.', '.', '.', '.', '.', '?', '!']);
        sentences.push(sentence(take(between(6, 16)), end));
      }
      blocks.push(sentences.join(' '));
    }
  }
  blocks.push(`${END_MARKER}.`);
  return { markdown: `${blocks.join('\n\n')}\n`, words: words + 3 };
}
