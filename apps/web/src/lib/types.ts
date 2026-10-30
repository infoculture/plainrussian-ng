// Типы ответа pr-wasm `analyzeLegacy`. Имена полей — как в JSON старого API.

export interface Metrics {
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

export interface Indexes {
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

export interface Measurement {
  metrics: Metrics;
  indexes: Indexes;
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
  terminated: boolean;
}

export interface Annotation {
  words: WordSpan[];
  sentences: SentenceSpan[];
}

export interface Analysis {
  measurement: Measurement;
  annotation: Annotation;
}
