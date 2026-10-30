// Типы ответов pr-wasm.

export type Algorithm = 'current' | 'legacy';

// ---- Текущий алгоритм: `analyze` ----

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

// ---- Разметка для подсветки, общая для обоих алгоритмов ----

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
  /** Только у старого алгоритма: предложение без точки в конце не считается. */
  terminated?: boolean;
}

export interface Annotation {
  words: WordSpan[];
  sentences: SentenceSpan[];
}

export interface Analysis {
  measurement: Measurement;
  annotation: Annotation;
}

// ---- Старый алгоритм: `analyzeLegacy`, имена полей как в JSON старого API ----

export interface LegacyMetrics {
  c_share: number;
  avg_slen: number;
  avg_syl: number;
  n_syllabes: number;
  n_words: number;
  n_sentences: number;
  n_complex_words: number;
  n_simple_words: number;
  chars: number;
  letters: number;
  spaces: number;
  wsyllabes: Record<string, number>;
}

export interface LegacyIndexes {
  index_fk: number;
  index_cl: number;
  index_dc: number;
  index_SMOG: number;
  index_ari: number;
  grade_fk: string;
  grade_cl: string;
  grade_dc: string;
  grade_SMOG: string;
  grade_ari: string;
}

export interface LegacyMeasurement {
  metrics: LegacyMetrics;
  indexes: LegacyIndexes;
}

export interface LegacyAnalysis {
  measurement: LegacyMeasurement;
  annotation: Annotation;
}

export type Result =
  | { algorithm: 'current'; analysis: Analysis }
  | { algorithm: 'legacy'; analysis: LegacyAnalysis };
