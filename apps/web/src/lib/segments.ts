import type { Annotation } from './types';

/** Слово сложное, если в нём больше 4 слогов — как в старом алгоритме. */
export const COMPLEX_SYLLABLES = 4;

/** Пороги подсветки длинных предложений, в словах. Это подсказка, а не часть метрики. */
export const LONG_SENTENCE = 20;
export const VERY_LONG_SENTENCE = 30;

/** Больше этого числа знаков подсветка не строится, чтобы не тормозила страница. */
export const HIGHLIGHT_LIMIT = 200_000;

export interface Part {
  text: string;
  /** Есть только у сложных слов. */
  syllables?: number;
}

export type Block =
  | { kind: 'gap'; text: string }
  | { kind: 'sentence'; level: 0 | 1 | 2; words: number; terminated: boolean; parts: Part[] };

/** Режет текст на промежутки и предложения, а предложения — на обычный текст и сложные слова. */
export function buildBlocks(text: string, annotation: Annotation): Block[] {
  const limit = Math.min(text.length, HIGHLIGHT_LIMIT);
  const complex = annotation.words.filter((w) => w.syllables > COMPLEX_SYLLABLES);
  const blocks: Block[] = [];
  let pos = 0;
  let w = 0;

  for (const s of annotation.sentences) {
    if (s.start >= limit) break;
    const end = Math.min(s.end, limit);
    if (s.start > pos) blocks.push({ kind: 'gap', text: text.slice(pos, s.start) });

    const parts: Part[] = [];
    let cursor = s.start;
    // Слово вроде «т.е.» может пересекать границу предложения: тогда оно
    // подсвечивается по частям в обоих предложениях.
    while (w < complex.length && complex[w].start < end) {
      const word = complex[w];
      const from = Math.max(word.start, cursor);
      const to = Math.min(word.end, end);
      if (from > cursor) parts.push({ text: text.slice(cursor, from) });
      if (to > from) parts.push({ text: text.slice(from, to), syllables: word.syllables });
      cursor = Math.max(cursor, to);
      if (word.end > end) break;
      w++;
    }
    if (end > cursor) parts.push({ text: text.slice(cursor, end) });

    const level = s.words >= VERY_LONG_SENTENCE ? 2 : s.words >= LONG_SENTENCE ? 1 : 0;
    blocks.push({ kind: 'sentence', level, words: s.words, terminated: s.terminated, parts });
    pos = end;
  }
  if (pos < limit) blocks.push({ kind: 'gap', text: text.slice(pos, limit) });
  return blocks;
}

/** Склонение: plural(5, ['слово', 'слова', 'слов']) → 'слов'. */
export function plural(n: number, forms: [string, string, string]): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return forms[0];
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return forms[1];
  return forms[2];
}
