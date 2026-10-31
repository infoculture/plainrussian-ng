//! Новый алгоритм оценки читаемости.
//!
//! Отличия от [`crate::legacy`]:
//! - предложения делит [`crate::sentenize`] (правила razdel), а не каждый `.?!`;
//!   предложения без слов (номера, разделители) не считаются;
//! - слово — буквы, внутри допустимы дефис и апостроф («из-за», «д’Артаньян»),
//!   знаки ударения слово не разрывают; числа считаются отдельно и словами не являются;
//! - слоги — гласные любого регистра, в латинице — группы гласных;
//!   аббревиатура из заглавных без гласных читается по буквам: «ВВП» — три слога.
//!
//! Формулы те же пять, но коэффициенты подобраны заново под этот подсчёт.
//! Они берутся из файла калибровки `calibration/<версия>.json`; версия
//! возвращается в каждом результате.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::legacy::grade_text;
use crate::sentenize::sentenize_chars;

/// Слово сложное, если в нём больше стольких слогов (как в старом алгоритме).
pub const COMPLEX_SYLLABLES: u64 = 4;

/// Коэффициенты формул. Каждая формула линейна: `y = Σ coef[i] * признак[i] + intercept`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Calibration {
    pub version: &'static str,
    /// Средняя ошибка сводной оценки на корпусе (LOO), лет обучения.
    pub average_error: f64,
    /// Flesch-Kincaid: слов в предложении, слогов в слове.
    pub fk: [f64; 3],
    /// Coleman-Liau: букв на 100 слов, предложений на 100 слов.
    pub cl: [f64; 3],
    /// SMOG: √(сложных слов на 30 предложений).
    pub smog: [f64; 2],
    /// Dale-Chale (упрощённая): сложных слов на 100 слов, слов в предложении.
    pub dc: [f64; 3],
    /// ARI: букв в слове, слов в предложении.
    pub ari: [f64; 3],
}

/// Текущая калибровка; значения совпадают с `calibration/2026.1.json`,
/// это проверяет тест `calibration_matches_json`.
pub const CALIBRATION: Calibration = Calibration {
    version: "2026.1",
    average_error: 1.308,
    fk: [0.2365, 16.9433, -34.6679],
    cl: [0.0609, -0.544, -21.2046],
    smog: [1.9837, -1.4327],
    dc: [1.0964, 0.1884, -1.3481],
    ari: [6.4631, 0.2559, -31.2201],
};

/// Счётчики текста.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Counts {
    pub sentences: u64,
    pub words: u64,
    pub syllables: u64,
    pub letters: u64,
    pub complex_words: u64,
    pub numbers: u64,
    /// Знаков без переводов строк.
    pub chars: u64,
    /// Число слогов в слове → число таких слов.
    pub words_by_syllables: BTreeMap<u64, u64>,
}

/// Признаки, на которых строятся формулы.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Features {
    /// Слов в предложении.
    pub words_per_sentence: f64,
    /// Слогов в слове.
    pub syllables_per_word: f64,
    /// Букв в слове.
    pub letters_per_word: f64,
    /// Сложных слов на 100 слов.
    pub complex_per_100_words: f64,
    /// Предложений на 100 слов.
    pub sentences_per_100_words: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Index {
    /// Лет обучения.
    pub value: f64,
    pub grade: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Indexes {
    /// Главная оценка — среднее пяти формул: по ошибке на корпусе она близка
    /// к лучшей формуле, а по максимальной ошибке лучше любой (`calibration/REPORT.md`).
    pub average: Index,
    pub fk: Index,
    pub cl: Index,
    pub smog: Index,
    pub dc: Index,
    pub ari: Index,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurement {
    pub calibration: Calibration,
    pub counts: Counts,
    /// Нет, если в тексте нет ни одного слова.
    pub features: Option<Features>,
    pub indexes: Option<Indexes>,
}

/// Слово для подсветки. Смещения — в единицах UTF-16, как у строк JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordSpan {
    pub start: usize,
    pub end: usize,
    pub syllables: u64,
}

/// Предложение для подсветки. Смещения — в единицах UTF-16.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentenceSpan {
    pub start: usize,
    pub end: usize,
    pub words: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Annotation {
    pub words: Vec<WordSpan>,
    pub sentences: Vec<SentenceSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    pub measurement: Measurement,
    pub annotation: Annotation,
}

pub fn measure(text: &str) -> Measurement {
    analyze(text).measurement
}

/// Считает показатели и размечает текст для подсветки.
pub fn analyze(text: &str) -> Analysis {
    let chars: Vec<char> = text.chars().collect();
    let words = scan_words(&chars);
    let sentences = sentenize_chars(&chars);

    let mut counts = Counts {
        chars: chars.iter().filter(|&&c| !matches!(c, '\n' | '\r')).count() as u64,
        numbers: count_numbers(&chars),
        ..Counts::default()
    };
    for w in &words {
        counts.words += 1;
        counts.syllables += w.syllables;
        counts.letters += w.letters;
        if w.syllables > COMPLEX_SYLLABLES {
            counts.complex_words += 1;
        }
        *counts.words_by_syllables.entry(w.syllables).or_insert(0) += 1;
    }

    // Слово относится к предложению, в котором начинается.
    let mut sentence_words = vec![0u64; sentences.len()];
    let mut i = 0;
    for w in &words {
        while i < sentences.len() && sentences[i].end <= w.start {
            i += 1;
        }
        if i < sentences.len() && sentences[i].start <= w.start {
            sentence_words[i] += 1;
        }
    }
    counts.sentences = sentence_words.iter().filter(|&&n| n > 0).count() as u64;

    let features = features(&counts);
    let indexes = features.map(|f| indexes(&f, &CALIBRATION));

    let utf16 = Utf16Offsets::new(&chars);
    let annotation = Annotation {
        words: words
            .iter()
            .map(|w| WordSpan {
                start: utf16.at(w.start),
                end: utf16.at(w.end),
                syllables: w.syllables,
            })
            .collect(),
        sentences: sentences
            .iter()
            .zip(&sentence_words)
            .map(|(s, &n)| SentenceSpan {
                start: utf16.at(s.start),
                end: utf16.at(s.end),
                words: n,
            })
            .collect(),
    };
    Analysis {
        measurement: Measurement {
            calibration: CALIBRATION,
            counts,
            features,
            indexes,
        },
        annotation,
    }
}

pub fn features(c: &Counts) -> Option<Features> {
    if c.words == 0 || c.sentences == 0 {
        return None;
    }
    let words = c.words as f64;
    let sentences = c.sentences as f64;
    Some(Features {
        words_per_sentence: words / sentences,
        syllables_per_word: c.syllables as f64 / words,
        letters_per_word: c.letters as f64 / words,
        complex_per_100_words: 100.0 * c.complex_words as f64 / words,
        sentences_per_100_words: 100.0 * sentences / words,
    })
}

pub fn indexes(f: &Features, k: &Calibration) -> Indexes {
    let index = |value: f64| Index {
        value,
        grade: grade_label(value),
    };
    // SMOG в классическом виде нормирован на 30 предложений.
    let smog_feature = (30.0 * f.complex_per_100_words / f.sentences_per_100_words).sqrt();
    let fk = k.fk[0] * f.words_per_sentence + k.fk[1] * f.syllables_per_word + k.fk[2];
    let cl = k.cl[0] * (100.0 * f.letters_per_word) + k.cl[1] * f.sentences_per_100_words + k.cl[2];
    let smog = k.smog[0] * smog_feature + k.smog[1];
    let dc = k.dc[0] * f.complex_per_100_words + k.dc[1] * f.words_per_sentence + k.dc[2];
    let ari = k.ari[0] * f.letters_per_word + k.ari[1] * f.words_per_sentence + k.ari[2];
    Indexes {
        average: index((fk + cl + smog + dc + ari) / 5.0),
        fk: index(fk),
        cl: index(cl),
        smog: index(smog),
        dc: index(dc),
        ari: index(ari),
    }
}

/// Словесное описание уровня. В отличие от старого алгоритма, всё, что проще
/// первого класса, попадает в младшую группу, а не в «неизвестно».
pub fn grade_label(value: f64) -> String {
    grade_text(value.max(1.0))
}

pub(crate) struct WordToken {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) syllables: u64,
    pub(crate) letters: u64,
}

/// Знак ударения и другие комбинирующие знаки не разрывают слово.
fn is_combining(ch: char) -> bool {
    matches!(ch, '\u{0300}'..='\u{036F}')
}

/// Дефис, апостроф и мягкий перенос внутри слова.
fn is_joiner(ch: char) -> bool {
    matches!(
        ch,
        '-' | '\u{2010}' | '\u{2011}' | '\'' | '\u{2019}' | '\u{02BC}' | '\u{00AD}'
    )
}

pub(crate) fn scan_words(chars: &[char]) -> Vec<WordToken> {
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        while end < chars.len() {
            let c = chars[end];
            if c.is_alphabetic() || is_combining(c) {
                end += 1;
            } else if is_joiner(c) && chars.get(end + 1).is_some_and(|n| n.is_alphabetic()) {
                end += 2;
            } else {
                break;
            }
        }
        let word = &chars[start..end];
        words.push(WordToken {
            start,
            end,
            syllables: syllables(word),
            letters: word.iter().filter(|c| c.is_alphabetic()).count() as u64,
        });
        i = end;
    }
    words
}

fn is_cyrillic_vowel(ch: char) -> bool {
    matches!(
        ch,
        'а' | 'е'
            | 'ё'
            | 'и'
            | 'о'
            | 'у'
            | 'ы'
            | 'э'
            | 'ю'
            | 'я'
            | 'А'
            | 'Е'
            | 'Ё'
            | 'И'
            | 'О'
            | 'У'
            | 'Ы'
            | 'Э'
            | 'Ю'
            | 'Я'
    )
}

fn is_latin_vowel(ch: char) -> bool {
    matches!(ch.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Слоги: каждая русская гласная и каждая группа латинских гласных подряд.
fn syllables(word: &[char]) -> u64 {
    let mut count = 0;
    let mut in_latin_vowels = false;
    for &c in word {
        if is_cyrillic_vowel(c) {
            count += 1;
        }
        let latin = is_latin_vowel(c);
        if latin && !in_latin_vowels {
            count += 1;
        }
        in_latin_vowels = latin;
    }
    let letters = word.iter().filter(|c| c.is_alphabetic()).count() as u64;
    // Аббревиатуру без гласных («ВВП», «МВД») читают по буквам.
    if count == 0 && letters >= 2 && word.iter().all(|c| !c.is_lowercase()) {
        return letters;
    }
    count
}

/// Числа: цифры, внутри допустимы точка и запятая («3.5», «1,25»).
fn count_numbers(chars: &[char]) -> u64 {
    let mut count = 0;
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        count += 1;
        while i < chars.len()
            && (chars[i].is_ascii_digit()
                || (matches!(chars[i], '.' | ',')
                    && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())))
        {
            i += 1;
        }
    }
    count
}

/// Перевод индексов символов в смещения UTF-16.
pub(crate) struct Utf16Offsets(Vec<usize>);

impl Utf16Offsets {
    pub(crate) fn new(chars: &[char]) -> Self {
        let mut offsets = Vec::with_capacity(chars.len() + 1);
        let mut pos = 0;
        offsets.push(0);
        for c in chars {
            pos += c.len_utf16();
            offsets.push(pos);
        }
        Self(offsets)
    }

    pub(crate) fn at(&self, char_index: usize) -> usize {
        self.0[char_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<(String, u64)> {
        let chars: Vec<char> = text.chars().collect();
        scan_words(&chars)
            .into_iter()
            .map(|w| (chars[w.start..w.end].iter().collect(), w.syllables))
            .collect()
    }

    #[test]
    fn words_and_syllables() {
        assert_eq!(
            words("ОБРАЗОВАНИЕ, Образование и из-за «д’Артаньяна» моло\u{301}ко ВВП в iPhone 3.5"),
            [
                ("ОБРАЗОВАНИЕ".to_string(), 6),
                ("Образование".to_string(), 6),
                ("и".to_string(), 1),
                ("из-за".to_string(), 2),
                ("д’Артаньяна".to_string(), 4),
                ("моло\u{301}ко".to_string(), 3),
                ("ВВП".to_string(), 3),
                ("в".to_string(), 0),
                ("iPhone".to_string(), 3),
            ]
        );
    }

    #[test]
    fn sentences_without_words_are_not_counted() {
        let m = measure("Глава 1.\n\n***\n\nМама мыла раму. Папа читал газету.");
        assert_eq!(m.counts.sentences, 3);
        assert_eq!(m.counts.words, 7);
        assert_eq!(m.counts.numbers, 1);
    }

    #[test]
    fn empty_text_has_no_indexes() {
        let m = measure("  123 ... ");
        assert_eq!(m.counts.words, 0);
        assert!(m.features.is_none() && m.indexes.is_none());
    }

    #[test]
    fn calibration_matches_json() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../calibration/2026.1.json")).unwrap();
        assert_eq!(json["version"], CALIBRATION.version);
        assert_eq!(json["average"]["loo"]["mae"], CALIBRATION.average_error);
        let coef = |key: &str| -> Vec<f64> {
            json["formulas"][key]["coef"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect()
        };
        assert_eq!(coef("fk"), CALIBRATION.fk);
        assert_eq!(coef("cl"), CALIBRATION.cl);
        assert_eq!(coef("smog"), CALIBRATION.smog);
        assert_eq!(coef("dc"), CALIBRATION.dc);
        assert_eq!(coef("ari"), CALIBRATION.ari);
    }

    #[test]
    fn grade_label_clamps_simple_texts() {
        assert_eq!(grade_label(-3.0), grade_text(1.0));
        assert_eq!(grade_label(30.0), grade_text(30.0));
    }
}
