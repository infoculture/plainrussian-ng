// Цель: для кого пишется текст. Вердикт сравнивает оценку с верхней границей
// выбранной аудитории, чтобы число лет обучения было с чем соотнести.
import { plural } from './segments.ts';

/** Границы шкалы оценки: корпус размечен от 1 до 17 («17 и выше»). */
export const SCALE_MIN = 1;
export const SCALE_MAX = 17;

export interface Audience {
  id: string;
  title: string;
  /** Верхняя граница, лет обучения: текст с такой оценкой аудитория ещё поймёт. */
  max: number;
}

/** Границы совпадают с уровнями шкалы (GRADE_TEXT в crates/pr-core/src/legacy.rs). */
export const AUDIENCES: Audience[] = [
  { id: 'primary', title: 'Младшие школьники', max: 3 },
  { id: 'middle', title: 'Школьники 4–6-го класса', max: 6 },
  { id: 'everyone', title: 'Широкая аудитория', max: 9 },
  { id: 'secondary', title: 'Читатели со средним образованием', max: 11 },
  { id: 'higher', title: 'Читатели с высшим образованием', max: 14 },
  // Не 17: тяжёлый канцелярит и законы трудны и специалистам.
  { id: 'experts', title: 'Специалисты', max: 16 },
];

const DEFAULT = 'everyone';
const KEY = 'plainrussian.audience';

function load(): string {
  try {
    const id = localStorage.getItem(KEY);
    if (id && AUDIENCES.some((a) => a.id === id)) return id;
  } catch {
    // Без хранилища остаётся аудитория по умолчанию.
  }
  return DEFAULT;
}

const target = $state({ id: load() });

/** Выбранная аудитория; общая для оценки формулами и оценки LLM. */
export function audience(): Audience {
  return AUDIENCES.find((a) => a.id === target.id) ?? AUDIENCES.find((a) => a.id === DEFAULT)!;
}

export function setAudience(id: string): void {
  target.id = id;
  try {
    localStorage.setItem(KEY, id);
  } catch {
    // Без хранилища выбор просто не запомнится.
  }
}

export interface Verdict {
  /** ok — подходит, close — превышение в пределах ошибки оценки, hard — сложнее цели. */
  level: 'ok' | 'close' | 'hard';
  text: string;
}

/**
 * Сравнивает оценку с целью. `tolerance` — ошибка оценки в годах обучения:
 * превышение в её пределах не называется «сложнее», а считается границей.
 */
export function verdict(grade: number, goal: Audience, tolerance: number): Verdict {
  const score = Math.min(Math.max(grade, SCALE_MIN), SCALE_MAX);
  const over = score - goal.max;
  if (over <= 0) return { level: 'ok', text: 'Подходит для выбранной аудитории.' };
  if (over <= tolerance) {
    return { level: 'close', text: 'На границе: чуть сложнее цели, разница меньше ошибки оценки.' };
  }
  const years = Math.max(1, Math.round(over));
  const by = `${years} ${plural(years, ['год', 'года', 'лет'])} обучения`;
  // Оценка упёрлась в верх шкалы: настоящая разница может быть больше.
  return {
    level: 'hard',
    text: score >= SCALE_MAX ? `Сложнее цели не меньше чем на ${by}.` : `Сложнее цели на ${by}.`,
  };
}
