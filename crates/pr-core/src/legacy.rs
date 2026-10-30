//! Алгоритм plainrussian.ru в том виде, в каком он работал в боевом API
//! (`plainrussian/api/textmetric/metric.py`, функция `calc_readability_metrics`).
//!
//! Перенос сделан один в один, вместе с известными ошибками:
//! - буквы и гласные считаются только строчными, поэтому «ОБРАЗОВАНИЕ»
//!   не считается словом, а у «Образование» на один слог меньше;
//! - каждый символ `.`, `?`, `!` считается концом предложения, поэтому
//!   «т.е.», «3.5» и многоточие дробят предложения.
//!
//! Коэффициенты формул подобраны под именно такой подсчёт. Поэтому ошибки
//! здесь не исправляются: исправленный подсчёт требует новой калибровки.
//! Совпадение с Python проверяют golden-тесты в `tests/legacy_golden.rs`.

use std::collections::BTreeMap;

use serde::Serialize;

// Пробелы для `str.split()` в Python 3: Unicode White_Space плюс `\x1c`–`\x1f`.
use crate::pychar::is_space as is_py_space;

/// Слово с числом слогов больше этого считается сложным.
pub const COMPLEX_SYL_FACTOR: u64 = 4;

// Flesch-Kincaid Grade, адаптированный для русского языка.
// 0.318 подобран на корпусе и с 1/π не связан.
#[allow(clippy::approx_constant)]
pub const FLG_X_GRADE: f64 = 0.318;
pub const FLG_Y_GRADE: f64 = 14.2;
pub const FLG_Z_GRADE: f64 = 30.5;

// Coleman-Liau.
pub const CLI_X_GRADE: f64 = 0.055;
pub const CLI_Y_GRADE: f64 = 0.35;
pub const CLI_Z_GRADE: f64 = 20.33;

// SMOG.
pub const SMOG_X_GRADE: f64 = 1.1;
pub const SMOG_Y_GRADE: f64 = 64.6;
pub const SMOG_Z_GRADE: f64 = 0.05;

// «Dale-Chale»: вместо списка знакомых слов используется доля сложных слов.
pub const DC_X_GRADE: f64 = 0.552;
pub const DC_Y_GRADE: f64 = 0.273;

// Automated Readability Index.
pub const ARI_X_GRADE: f64 = 6.26;
pub const ARI_Y_GRADE: f64 = 0.2805;
pub const ARI_Z_GRADE: f64 = 31.04;

const GRADE_TEXT: [&str; 17] = [
    "1 - 3-й класс (возраст примерно: 6-8 лет)",
    "1 - 3-й класс (возраст примерно: 6-8 лет)",
    "1 - 3-й класс (возраст примерно: 6-8 лет)",
    "4 - 6-й класс (возраст примерно: 9-11 лет)",
    "4 - 6-й класс (возраст примерно: 9-11 лет)",
    "4 - 6-й класс (возраст примерно: 9-11 лет)",
    "7 - 9-й класс (возраст примерно: 12-14 лет)",
    "7 - 9-й класс (возраст примерно: 12-14 лет)",
    "7 - 9-й класс (возраст примерно: 12-14 лет)",
    "10 - 11-й класс (возраст примерно: 15-16 лет)",
    "10 - 11-й класс (возраст примерно: 15-16 лет)",
    "1 - 3 курсы ВУЗа (возраст примерно: 17-19 лет)",
    "1 - 3 курсы ВУЗа (возраст примерно: 17-19 лет)",
    "1 - 3 курсы ВУЗа (возраст примерно: 17-19 лет)",
    "4 - 6 курсы ВУЗа (возраст примерно: 20-22 лет)",
    "4 - 6 курсы ВУЗа (возраст примерно: 20-22 лет)",
    "4 - 6 курсы ВУЗа (возраст примерно: 20-22 лет)",
];

const POST_GRADE_TEXT_18_24: &str = "Аспирантура, второе высшее образование, phD";

/// Расчётные показатели текста. Имена полей совпадают с JSON старого API,
/// включая опечатку `n_syllabes`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metrics {
    /// Доля сложных слов, в процентах.
    pub c_share: f64,
    /// Среднее число слов на предложение.
    pub avg_slen: f64,
    /// Среднее число слогов на слово.
    pub avg_syl: f64,
    pub n_syllabes: u64,
    pub n_words: u64,
    pub n_sentences: u64,
    pub n_complex_words: u64,
    pub n_simple_words: u64,
    pub chars: u64,
    pub letters: u64,
    pub spaces: u64,
    /// Число слогов в слове → число таких слов.
    pub wsyllabes: BTreeMap<u64, u64>,
}

/// Индексы читаемости в годах обучения и их словесные описания.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Indexes {
    pub index_fk: f64,
    pub index_cl: f64,
    pub index_dc: f64,
    #[serde(rename = "index_SMOG")]
    pub index_smog: f64,
    pub index_ari: f64,
    pub grade_fk: String,
    pub grade_cl: String,
    pub grade_dc: String,
    #[serde(rename = "grade_SMOG")]
    pub grade_smog: String,
    pub grade_ari: String,
}

/// Результат в формате ответа `/api/1.0/ru/measure/` без полей `status` и `debug`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurement {
    pub metrics: Metrics,
    pub indexes: Indexes,
}

pub fn calc_flesch_kincaid_grade(n_syllabes: u64, n_words: u64, n_sent: u64) -> f64 {
    if n_words == 0 || n_sent == 0 {
        return 0.0;
    }
    let (syl, words, sent) = (n_syllabes as f64, n_words as f64, n_sent as f64);
    FLG_X_GRADE * (words / sent) + FLG_Y_GRADE * (syl / words) - FLG_Z_GRADE
}

pub fn calc_coleman_liau_index(n_letters: u64, n_words: u64, n_sent: u64) -> f64 {
    if n_words == 0 {
        return 0.0;
    }
    let (letters, words, sent) = (n_letters as f64, n_words as f64, n_sent as f64);
    CLI_X_GRADE * (letters * (100.0 / words)) - CLI_Y_GRADE * (sent * (100.0 / words)) - CLI_Z_GRADE
}

pub fn calc_dale_chale(n_complex: u64, n_words: u64, n_sent: u64) -> f64 {
    if n_words == 0 || n_sent == 0 {
        return 0.0;
    }
    let (complex, words, sent) = (n_complex as f64, n_words as f64, n_sent as f64);
    DC_X_GRADE * (100.0 * complex / words) + DC_Y_GRADE * (words / sent)
}

pub fn calc_smog_index(n_complex: u64, n_sent: u64) -> f64 {
    if n_sent == 0 {
        return 0.0;
    }
    let (complex, sent) = (n_complex as f64, n_sent as f64);
    SMOG_X_GRADE * ((SMOG_Y_GRADE / sent) * complex).sqrt() + SMOG_Z_GRADE
}

pub fn calc_ari_index(n_letters: u64, n_words: u64, n_sent: u64) -> f64 {
    if n_words == 0 || n_sent == 0 {
        return 0.0;
    }
    let (letters, words, sent) = (n_letters as f64, n_words as f64, n_sent as f64);
    ARI_X_GRADE * (letters / words) + ARI_Y_GRADE * (words / sent) - ARI_Z_GRADE
}

/// Словесное описание уровня по индексу (`add_grade_text` в Python).
pub fn grade_text(index: f64) -> String {
    // Боевой API работал на Python 2: round() округлял половины от нуля, как f64::round.
    let grade = index.round();
    if (1.0..=17.0).contains(&grade) {
        GRADE_TEXT[grade as usize - 1].to_string()
    } else if grade > 17.0 {
        POST_GRADE_TEXT_18_24.to_string()
    } else {
        format!("неизвестно ({})", grade as i64)
    }
}

/// Считает показатели и индексы текста так же, как старый API.
pub fn measure(text: &str) -> Measurement {
    let mut c = Counts::default();
    let mut wsyl = 0;
    // Python обходит text.splitlines(), а внутри строки делит её через split().
    // Все границы строк — пробельные символы для split(), поэтому один проход
    // по тексту даёт те же слова. Сами границы строк в chars не входят.
    for ch in text.chars() {
        if !is_py_line_boundary(ch) {
            c.chars += 1;
        }
        if matches!(ch, '.' | '?' | '!') {
            c.sentences += 1;
        }
        if matches!(ch, ' ' | '\t') {
            c.spaces += 1;
        }
        if is_py_space(ch) {
            c.finish_word(wsyl);
            wsyl = 0;
            continue;
        }
        if is_ru_letter(ch) {
            c.letters += 1;
        }
        if is_ru_vowel(ch) {
            c.syllabes += 1;
            wsyl += 1;
        }
    }
    c.finish_word(wsyl);
    c.into_measurement()
}

#[derive(Default)]
struct Counts {
    sentences: u64,
    chars: u64,
    spaces: u64,
    letters: u64,
    syllabes: u64,
    words: u64,
    complex_words: u64,
    simple_words: u64,
    wsyllabes: BTreeMap<u64, u64>,
}

impl Counts {
    /// Слово без гласных не считается ни словом, ни простым, ни сложным.
    fn finish_word(&mut self, wsyl: u64) {
        if wsyl == 0 {
            return;
        }
        if wsyl > COMPLEX_SYL_FACTOR {
            self.complex_words += 1;
        } else {
            self.simple_words += 1;
        }
        self.words += 1;
        *self.wsyllabes.entry(wsyl).or_insert(0) += 1;
    }

    fn into_measurement(self) -> Measurement {
        let words = self.words as f64;
        let sentences = self.sentences as f64;
        let metrics = Metrics {
            c_share: if self.words > 0 {
                self.complex_words as f64 * 100.0 / words
            } else {
                0.0
            },
            avg_slen: if self.sentences > 0 {
                words / sentences
            } else {
                0.0
            },
            avg_syl: if self.words > 0 {
                self.syllabes as f64 / words
            } else {
                0.0
            },
            n_syllabes: self.syllabes,
            n_words: self.words,
            n_sentences: self.sentences,
            n_complex_words: self.complex_words,
            n_simple_words: self.simple_words,
            chars: self.chars,
            letters: self.letters,
            spaces: self.spaces,
            wsyllabes: self.wsyllabes,
        };
        let index_fk = calc_flesch_kincaid_grade(self.syllabes, self.words, self.sentences);
        let index_cl = calc_coleman_liau_index(self.letters, self.words, self.sentences);
        let index_dc = calc_dale_chale(self.complex_words, self.words, self.sentences);
        let index_smog = calc_smog_index(self.complex_words, self.sentences);
        let index_ari = calc_ari_index(self.letters, self.words, self.sentences);
        let indexes = Indexes {
            index_fk,
            index_cl,
            index_dc,
            index_smog,
            index_ari,
            grade_fk: grade_text(index_fk),
            grade_cl: grade_text(index_cl),
            grade_dc: grade_text(index_dc),
            grade_smog: grade_text(index_smog),
            grade_ari: grade_text(index_ari),
        };
        Measurement { metrics, indexes }
    }
}

/// Слово в смысле старого алгоритма: отрезок между пробелами, в котором есть
/// строчная гласная. Границы сужены до первой и последней буквы, чтобы
/// подсветка не захватывала кавычки и знаки препинания.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordSpan {
    pub start: usize,
    pub end: usize,
    pub syllables: u64,
}

/// Предложение в смысле старого алгоритма: текст до очередного `.`, `?` или `!`
/// включительно. Хвост без такого знака помечен `terminated: false` и в
/// `n_sentences` не входит.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentenceSpan {
    pub start: usize,
    pub end: usize,
    /// Число слов, которые начинаются в этом предложении.
    pub words: u64,
    pub terminated: bool,
}

/// Разметка текста для подсветки. Смещения — в единицах UTF-16, как у строк
/// JavaScript. Счёт совпадает с [`measure`]: столько же слов, слогов и
/// закрытых предложений.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Annotation {
    pub words: Vec<WordSpan>,
    pub sentences: Vec<SentenceSpan>,
}

/// Размечает слова и предложения по тем же правилам, что и [`measure`].
pub fn annotate(text: &str) -> Annotation {
    let mut words = Vec::new();
    let mut sentences = Vec::new();
    let mut pos = 0;
    let mut wsyl = 0;
    // Первая и последняя буква текущего слова.
    let mut letters: Option<(usize, usize)> = None;
    let mut sentence_start: Option<usize> = None;
    let mut last_visible_end = 0;

    for ch in text.chars() {
        let next = pos + ch.len_utf16();
        if is_py_space(ch) {
            push_word(&mut words, wsyl, letters.take());
            wsyl = 0;
        } else {
            let start = *sentence_start.get_or_insert(pos);
            last_visible_end = next;
            if ch.is_alphabetic() {
                letters = Some((letters.map_or(pos, |(first, _)| first), next));
            }
            if is_ru_vowel(ch) {
                wsyl += 1;
            }
            if matches!(ch, '.' | '?' | '!') {
                sentences.push(SentenceSpan {
                    start,
                    end: next,
                    words: 0,
                    terminated: true,
                });
                sentence_start = None;
            }
        }
        pos = next;
    }
    push_word(&mut words, wsyl, letters);
    if let Some(start) = sentence_start {
        sentences.push(SentenceSpan {
            start,
            end: last_visible_end,
            words: 0,
            terminated: false,
        });
    }

    // Начало слова — буква, а каждый непробельный символ лежит в каком-то
    // предложении, поэтому подходящее предложение всегда найдётся.
    let mut i = 0;
    for word in &words {
        while sentences[i].end <= word.start {
            i += 1;
        }
        sentences[i].words += 1;
    }
    Annotation { words, sentences }
}

fn push_word(words: &mut Vec<WordSpan>, wsyl: u64, letters: Option<(usize, usize)>) {
    if wsyl == 0 {
        return;
    }
    let (start, end) = letters.expect("в слове с гласной есть буква");
    words.push(WordSpan {
        start,
        end,
        syllables: wsyl,
    });
}

/// Только строчные буквы: в Python `RU_LETTERS` нет заглавных.
fn is_ru_letter(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'ё')
}

/// Только строчные гласные, как `RU_VOWELS` в Python.
fn is_ru_vowel(ch: char) -> bool {
    matches!(
        ch,
        'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я'
    )
}

/// Границы строк для `str.splitlines()` в Python 3. В Python 2.7 `\v` и `\f`
/// границами не были (их добавили в 3.2), это может давать разницу в `chars`.
fn is_py_line_boundary(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r'
            | '\x0b'
            | '\x0c'
            | '\x1c'
            | '\x1d'
            | '\x1e'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotate_spans_in_utf16() {
        // 👋 занимает две единицы UTF-16, поэтому «мир» начинается с 12, а не с 11.
        let text = "«Привет» 👋 мир! Высокопревосходительство т.е. спит";
        let a = annotate(text);
        let spans: Vec<_> = a
            .words
            .iter()
            .map(|w| (w.start, w.end, w.syllables))
            .collect();
        // «Привет» без кавычек; «т.е.» — одно слово с одной гласной.
        assert_eq!(
            spans,
            [
                (1, 7, 2),
                (12, 15, 1),
                (17, 41, 9),
                (42, 45, 1),
                (47, 51, 1)
            ]
        );
        let sentences: Vec<_> = a
            .sentences
            .iter()
            .map(|s| (s.start, s.end, s.words, s.terminated))
            .collect();
        // «т.е.» дробит предложение, как в старом алгоритме.
        assert_eq!(
            sentences,
            [
                (0, 16, 2, true),
                (17, 44, 2, true),
                (44, 46, 0, true),
                (47, 51, 1, false)
            ]
        );
    }
}
