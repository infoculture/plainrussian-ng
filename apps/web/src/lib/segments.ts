import type { Annotation, SentenceSpan } from './types';

/** Слово сложное, если в нём больше 4 слогов — как в ядре. */
export const COMPLEX_SYLLABLES = 4;

/** Пороги подсветки длинных предложений, в словах. Это подсказка, а не часть метрики. */
export const LONG_SENTENCE = 20;
export const VERY_LONG_SENTENCE = 30;

/** Больше этого числа знаков подсветка не строится, чтобы не тормозила страница. */
export const HIGHLIGHT_LIMIT = 200_000;

/** Отмеченный фрагмент текста. Смещения — в единицах UTF-16. */
export interface Mark {
  start: number;
  end: number;
  /** CSS-класс отметки. */
  cls: string;
  title: string;
}

export interface Part {
  text: string;
  mark?: { cls: string; title: string };
}

export type Block =
  | { kind: 'gap'; text: string }
  | { kind: 'sentence'; level: 0 | 1 | 2; words: number; parts: Part[] };

export function complexWordMarks(annotation: Annotation): Mark[] {
  return annotation.words
    .filter((w) => w.syllables > COMPLEX_SYLLABLES)
    .map((w) => ({
      start: w.start,
      end: w.end,
      cls: 'complex',
      title: `${w.syllables} ${plural(w.syllables, ['слог', 'слога', 'слогов'])}`,
    }));
}

/**
 * Режет текст на промежутки и предложения, а предложения — на обычный текст и отметки.
 * Отметки должны идти по порядку и не пересекаться.
 */
export function buildBlocks(text: string, sentences: SentenceSpan[], marks: Mark[]): Block[] {
  const limit = Math.min(text.length, HIGHLIGHT_LIMIT);
  const blocks: Block[] = [];
  let pos = 0;
  let m = 0;

  for (const s of sentences) {
    if (s.start >= limit) break;
    const end = Math.min(s.end, limit);
    if (s.start > pos) blocks.push({ kind: 'gap', text: text.slice(pos, s.start) });

    const parts: Part[] = [];
    let cursor = s.start;
    // Отметка может пересекать границу предложения: тогда она подсвечивается
    // по частям в обоих предложениях.
    while (m < marks.length && marks[m].start < end) {
      const mark = marks[m];
      const from = Math.max(mark.start, cursor);
      const to = Math.min(mark.end, end);
      if (from > cursor) parts.push({ text: text.slice(cursor, from) });
      if (to > from) parts.push({ text: text.slice(from, to), mark: { cls: mark.cls, title: mark.title } });
      cursor = Math.max(cursor, to);
      if (mark.end > end) break;
      m++;
    }
    if (end > cursor) parts.push({ text: text.slice(cursor, end) });

    const level = s.words >= VERY_LONG_SENTENCE ? 2 : s.words >= LONG_SENTENCE ? 1 : 0;
    blocks.push({ kind: 'sentence', level, words: s.words, parts });
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
