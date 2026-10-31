// Типы ответа pr-wasm `analyze`.


export interface Calibration {
  version: string;
  /** Средняя ошибка сводной оценки на корпусе, лет обучения. */
  average_error: number;
}

export interface Counts {
  sentences: number;
  words: number;
  syllables: number;
  letters: number;
  complex_words: number;
  numbers: number;
  chars: number;
  words_by_syllables: Record<string, number>;
}

export interface Features {
  words_per_sentence: number;
  syllables_per_word: number;
  letters_per_word: number;
  complex_per_100_words: number;
  sentences_per_100_words: number;
}

export interface Index {
  value: number;
  grade: string;
}

export interface Indexes {
  average: Index;
  fk: Index;
  cl: Index;
  smog: Index;
  dc: Index;
  ari: Index;
}

export interface Measurement {
  calibration: Calibration;
  counts: Counts;
  /** null, если в тексте нет слов. */
  features: Features | null;
  indexes: Indexes | null;
}

/** Смещения — в единицах UTF-16, подходят для `String.slice`. */
export interface WordSpan {
  start: number;
  end: number;
  syllables: number;
}

export interface SentenceSpan {
  start: number;
  end: number;
  words: number;
}

export interface Annotation {
  words: WordSpan[];
  sentences: SentenceSpan[];
}

export interface Analysis {
  measurement: Measurement;
  annotation: Annotation;
}

// ---- Проверки ясного языка: `Clarity.check` ----

export type ClarityKind = 'nominalization' | 'passive' | 'genitive_chain' | 'bureaucratese' | 'abbreviation';

/** Смещения — в единицах UTF-16. */
export interface Finding {
  kind: ClarityKind;
  start: number;
  end: number;
  text: string;
}

export interface ClarityReport {
  words: number;
  counts: Record<ClarityKind, number>;
  findings: Finding[];
}
