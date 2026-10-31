// Проверки ясного языка: подписи, советы и отметки для подсветки.
import type { Mark } from './segments';
import type { ClarityKind, ClarityReport } from './types';

export const CLARITY_KINDS: { kind: ClarityKind; label: string; advice: string }[] = [
  {
    kind: 'genitive_chain',
    label: 'Цепочки родительных падежей',
    advice: 'Разбейте цепочку или перестройте фразу: «повышение эффективности работы органов» → «чтобы органы работали лучше».',
  },
  {
    kind: 'nominalization',
    label: 'Отглагольные существительные',
    advice: 'Замените глаголом: «проведение проверки» → «проверить», «осуществление контроля» → «контролировать».',
  },
  {
    kind: 'bureaucratese',
    label: 'Канцеляризмы',
    advice: 'Скажите проще: «в целях» → «чтобы», «осуществлять» → «делать», «является» → «это».',
  },
  {
    kind: 'passive',
    label: 'Страдательный залог',
    advice: 'Назовите, кто действует: «утверждённый министерством порядок» → «порядок, который утвердило министерство».',
  },
  {
    kind: 'abbreviation',
    label: 'Аббревиатуры',
    advice: 'Расшифруйте при первом упоминании: не все читатели их знают.',
  },
];

export function clarityLabel(kind: ClarityKind): string {
  return CLARITY_KINDS.find((k) => k.kind === kind)?.label ?? kind;
}

/**
 * Отметки для подсветки. Находки могут вкладываться друг в друга (отглагольное
 * существительное внутри цепочки): остаётся внешняя, затем следующая после неё.
 */
export function clarityMarks(report: ClarityReport): Mark[] {
  const sorted = [...report.findings].sort((a, b) => a.start - b.start || b.end - a.end);
  const marks: Mark[] = [];
  let end = 0;
  for (const f of sorted) {
    if (f.start < end) continue;
    marks.push({ start: f.start, end: f.end, cls: `clarity-${f.kind}`, title: clarityLabel(f.kind) });
    end = f.end;
  }
  return marks;
}

const KEY = 'plainrussian.clarity';

/** Включённая проверка — удобство одного зрителя: словарь уже в кеше браузера. */
export function clarityEnabled(): boolean {
  try {
    return localStorage.getItem(KEY) === 'on';
  } catch {
    return false;
  }
}

export function setClarityEnabled(on: boolean): void {
  try {
    if (on) localStorage.setItem(KEY, 'on');
    else localStorage.removeItem(KEY);
  } catch {
    // Без хранилища выбор просто не запомнится.
  }
}
