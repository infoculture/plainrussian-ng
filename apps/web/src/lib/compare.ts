// Сравнение двух вариантов текста по показателям формул: для «Было и стало» и для
// черновика LLM.
import { plural } from './segments';
import { SCALE_MAX, SCALE_MIN } from './target.svelte.ts';
import type { Measurement } from './types';

const fmt = (n: number) => n.toLocaleString('ru-RU', { maximumFractionDigits: 1 });
const clamp = (value: number) => Math.min(Math.max(value, SCALE_MIN), SCALE_MAX);

export interface Facts {
  grade: number;
  sentence: number;
  complex: number;
}

/** Что нужно для сравнения; null, если в варианте нет слов. */
export function facts(m: Measurement | null): Facts | null {
  return m?.indexes && m.features
    ? {
        grade: m.indexes.average.value,
        sentence: m.features.words_per_sentence,
        complex: m.features.complex_per_100_words,
      }
    : null;
}

export interface Change {
  tone: 'ok' | 'hard' | 'same';
  text: string;
}

/** Сравнивает оценки в пределах шкалы: за её границами числа смысла не имеют. */
export function change(from: number, to: number): Change {
  const diff = clamp(to) - clamp(from);
  const size = Math.round(Math.abs(diff) * 10) / 10;
  if (size === 0) {
    // Обе оценки выше шкалы: на шкале то же «17+», хотя текст изменился.
    if (from >= SCALE_MAX && to >= SCALE_MAX && Math.abs(to - from) >= 0.05) {
      return to < from
        ? { tone: 'ok', text: 'Стало проще, но оценка всё ещё выше шкалы.' }
        : { tone: 'hard', text: 'Стало сложнее, оценка по-прежнему выше шкалы.' };
    }
    return { tone: 'same', text: 'Оценка не изменилась.' };
  }
  // Дробное число требует родительного падежа единственного числа: «на 4,6 года».
  const unit = Number.isInteger(size) ? plural(size, ['год', 'года', 'лет']) : 'года';
  const by = `${fmt(size)} ${unit} обучения`;
  return diff < 0
    ? { tone: 'ok', text: `Стало проще на ${by}.` }
    : { tone: 'hard', text: `Стало сложнее на ${by}.` };
}
