//! Проверки ясного языка: приёмы, которые делают текст тяжелее, чем нужно.
//!
//! - отглагольные существительные («проведение мониторинга» вместо «проверять»);
//! - страдательные причастия («проведённого», «утверждённые»);
//! - цепочки из трёх и больше существительных в родительном падеже;
//! - канцеляризмы: «в целях», «в рамках», «осуществлять», «являться»;
//! - аббревиатуры: их стоит расшифровать при первом упоминании.
//!
//! Части речи и падежи определяет морфологический анализатор rumorph со словарём
//! OpenCorpora; падеж выбирается с учётом соседних слов. Словарь передаёт вызывающий
//! код: в программах он встроен, в браузере загружается отдельно.
//!
//! Канцеляризмы и их замены — в `data/bureaucratese.txt`.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Serialize;

use crate::readability::{Utf16Offsets, WordToken, scan_words};
use crate::sentenize::sentenize_chars;

pub use rumorph::Dictionary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Nominalization,
    Passive,
    GenitiveChain,
    Bureaucratese,
    Abbreviation,
}

/// Найденный фрагмент. Смещения — в единицах UTF-16, как у строк JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub text: String,
    /// Чем заменить: есть у канцеляризмов из словаря.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<&'static str>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Counts {
    pub nominalization: u64,
    pub passive: u64,
    pub genitive_chain: u64,
    pub bureaucratese: u64,
    pub abbreviation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub words: u64,
    pub counts: Counts,
    /// В порядке появления в тексте.
    pub findings: Vec<Finding>,
}

/// Цепочкой родительных считается столько существительных подряд и больше.
pub const GENITIVE_CHAIN: usize = 3;

/// Словарь канцеляризмов с заменами, `data/bureaucratese.txt`.
#[derive(Debug, Default)]
struct Bureaucratese {
    /// Начальная форма слова → замена.
    words: HashMap<String, &'static str>,
    /// Обороты по словоформам подряд.
    forms: Vec<(Vec<String>, &'static str)>,
    /// Обороты по начальным формам подряд.
    lemmas: Vec<(Vec<String>, &'static str)>,
}

static BUREAUCRATESE: LazyLock<Bureaucratese> = LazyLock::new(|| {
    parse_bureaucratese(include_str!("../data/bureaucratese.txt"))
        .unwrap_or_else(|e| panic!("data/bureaucratese.txt: {e}"))
});

/// Разбирает словарь канцеляризмов. Ошибка — с номером строки.
fn parse_bureaucratese(source: &'static str) -> Result<Bureaucratese, String> {
    let mut dict = Bureaucratese::default();
    let mut section = None;
    for (n, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = Some(name);
            continue;
        }
        let at = |message: &str| format!("строка {}: {message}", n + 1);
        let (pattern, suggestion) = line
            .split_once(" = ")
            .ok_or_else(|| at("нужно «шаблон = замена»"))?;
        let words: Vec<String> = pattern.split_whitespace().map(normalize).collect();
        let suggestion = suggestion.trim();
        if words.is_empty() || suggestion.is_empty() {
            return Err(at("пустой шаблон или замена"));
        }
        let duplicate = match section {
            Some("слова") if words.len() == 1 => {
                dict.words.insert(words[0].clone(), suggestion).is_some()
            }
            Some("слова") => return Err(at("в [слова] — одно слово")),
            Some("обороты") => push_phrase(&mut dict.forms, words, suggestion),
            Some("обороты по начальным формам") => {
                push_phrase(&mut dict.lemmas, words, suggestion)
            }
            Some(other) => return Err(at(&format!("неизвестный раздел [{other}]"))),
            None => return Err(at("строка до первого раздела")),
        };
        if duplicate {
            return Err(at(&format!("«{pattern}» уже есть")));
        }
    }
    Ok(dict)
}

/// Добавляет оборот; true, если такой уже был.
fn push_phrase(
    list: &mut Vec<(Vec<String>, &'static str)>,
    words: Vec<String>,
    suggestion: &'static str,
) -> bool {
    let duplicate = list.iter().any(|(w, _)| *w == words);
    list.push((words, suggestion));
    duplicate
}

/// Сравнение без учёта регистра и разницы между «е» и «ё».
fn normalize(word: &str) -> String {
    word.to_lowercase().replace('ё', "е")
}

/// Существительные на -ние и -тие, которые обычно не канцелярит: предметы и
/// привычные слова. Остальные отмечаются как отглагольные.
const NOT_NOMINALIZATIONS: &[&str] = &[
    "внимание",
    "желание",
    "задание",
    "занятие",
    "здание",
    "знание",
    // Не отглагольное, но канцелярит: есть в data/bureaucratese.txt.
    "мероприятие",
    "мнение",
    "настроение",
    "образование",
    "растение",
    "событие",
    "сознание",
];

/// Проверяет текст. Словарь создаётся один раз и используется для всех текстов.
pub fn check(dictionary: &Dictionary, text: &str) -> Report {
    let chars: Vec<char> = text.chars().collect();
    let words = scan_words(&chars);
    let sentences = sentenize_chars(&chars);
    let utf16 = Utf16Offsets::new(&chars);
    let mut findings = Vec::new();

    let mut next = 0;
    for sentence in &sentences {
        let first = next;
        while next < words.len() && words[next].start < sentence.end {
            next += 1;
        }
        let sentence_words = &words[first..next];
        if sentence_words.is_empty() {
            continue;
        }
        let forms: Vec<String> = sentence_words
            .iter()
            .map(|w| chars[w.start..w.end].iter().collect())
            .collect();
        let parsed = parse(dictionary, &forms);
        let ctx = Sentence {
            chars: &chars,
            words: sentence_words,
            forms: &forms,
            parsed: &parsed,
        };
        ctx.find(&mut findings);
    }

    findings.sort_by_key(|f| (f.start, f.kind));
    let mut counts = Counts::default();
    let findings = findings
        .into_iter()
        .map(
            |Raw {
                 kind,
                 start,
                 end,
                 suggestion,
             }| {
                *match kind {
                    Kind::Nominalization => &mut counts.nominalization,
                    Kind::Passive => &mut counts.passive,
                    Kind::GenitiveChain => &mut counts.genitive_chain,
                    Kind::Bureaucratese => &mut counts.bureaucratese,
                    Kind::Abbreviation => &mut counts.abbreviation,
                } += 1;
                Finding {
                    kind,
                    start: utf16.at(start),
                    end: utf16.at(end),
                    text: chars[start..end].iter().collect(),
                    suggestion,
                }
            },
        )
        .collect();
    Report {
        words: words.len() as u64,
        counts,
        findings,
    }
}

/// Разбор слова, нужный проверкам.
struct Parsed {
    lemma: String,
    pos: String,
    case: Option<String>,
    passive: bool,
    abbreviation: bool,
}

fn parse(dictionary: &Dictionary, forms: &[String]) -> Vec<Parsed> {
    // Разбор с контекстом точнее; если модели нет, хватит разбора по словарю.
    let analyses = dictionary
        .analyze_sentence(forms)
        .unwrap_or_else(|_| forms.iter().map(|f| dictionary.analyze(f)).collect());
    analyses
        .into_iter()
        .zip(forms)
        .map(|(variants, form)| {
            let best = variants.into_iter().next();
            let tag = best.as_ref().and_then(|a| dictionary.ud_tag(a.ud));
            // Заглавными пишут и аббревиатуры, и обычные слова («ИЛИ» в юридическом тексте).
            // Аббревиатура — если словарь так её и разбирает или не знает такого слова.
            let abbreviation = is_all_caps(form)
                && (tag.is_some_and(|t| t.feat("Abbr") == Some("Yes"))
                    || !dictionary.is_known(&form.to_lowercase()));
            Parsed {
                lemma: best.map(|a| a.lemma).unwrap_or_default(),
                pos: tag.map(|t| t.pos().to_string()).unwrap_or_default(),
                case: tag.and_then(|t| t.feat("Case")).map(str::to_string),
                passive: tag.is_some_and(|t| t.feat("Voice") == Some("Pass")),
                abbreviation,
            }
        })
        .collect()
}

struct Sentence<'a> {
    chars: &'a [char],
    words: &'a [WordToken],
    forms: &'a [String],
    parsed: &'a [Parsed],
}

/// Находка до перевода смещений: индексы символов.
struct Raw {
    kind: Kind,
    start: usize,
    end: usize,
    suggestion: Option<&'static str>,
}

impl Raw {
    fn new(kind: Kind, start: usize, end: usize) -> Self {
        Self {
            kind,
            start,
            end,
            suggestion: None,
        }
    }
}

impl Sentence<'_> {
    fn find(&self, out: &mut Vec<Raw>) {
        let dict = &*BUREAUCRATESE;
        // Обороты первыми: слово из словаря внутри оборота («данный» в «на данный
        // момент») — часть той же находки, отдельно оно не считается.
        let phrases_from = out.len();
        self.find_phrases(dict, out);
        let phrases: Vec<(usize, usize)> = out[phrases_from..]
            .iter()
            .map(|f| (f.start, f.end))
            .collect();
        for (word, p) in self.words.iter().zip(self.parsed) {
            if p.abbreviation {
                out.push(Raw::new(Kind::Abbreviation, word.start, word.end));
            }
            if p.pos == "NOUN" && is_nominalization(&p.lemma) {
                out.push(Raw::new(Kind::Nominalization, word.start, word.end));
            }
            if p.pos == "VERB" && p.passive {
                out.push(Raw::new(Kind::Passive, word.start, word.end));
            }
            let inside = phrases
                .iter()
                .any(|&(s, e)| s <= word.start && word.end <= e);
            if let Some(&suggestion) = dict.words.get(&normalize(&p.lemma))
                && !inside
            {
                out.push(Raw {
                    suggestion: Some(suggestion),
                    ..Raw::new(Kind::Bureaucratese, word.start, word.end)
                });
            }
        }
        self.find_genitive_chains(out);
    }

    /// Между словами только пробелы: знак препинания разрывает оборот и цепочку.
    fn adjacent(&self, i: usize) -> bool {
        self.chars[self.words[i].end..self.words[i + 1].start]
            .iter()
            .all(|c| c.is_whitespace())
    }

    /// Обороты по словоформам и по начальным формам. Обороты одного вида не пересекаются:
    /// из двух, что начинаются с одного слова, берётся длинный, а следующий ищется после него.
    fn find_phrases(&self, dict: &Bureaucratese, out: &mut Vec<Raw>) {
        let forms: Vec<String> = self.forms.iter().map(|f| normalize(f)).collect();
        let lemmas: Vec<String> = self.parsed.iter().map(|p| normalize(&p.lemma)).collect();
        let mut start = 0;
        while start < forms.len() {
            let longest = [(&forms, &dict.forms), (&lemmas, &dict.lemmas)]
                .into_iter()
                .flat_map(|(text, phrases)| phrases.iter().map(move |p| (text, p)))
                .filter(|(text, (words, _))| {
                    let end = start + words.len();
                    end <= text.len()
                        && text[start..end] == words[..]
                        && (start..end - 1).all(|i| self.adjacent(i))
                })
                .map(|(_, (words, suggestion))| (words.len(), *suggestion))
                .max_by_key(|&(len, _)| len);
            match longest {
                Some((len, suggestion)) => {
                    out.push(Raw {
                        suggestion: Some(suggestion),
                        ..Raw::new(
                            Kind::Bureaucratese,
                            self.words[start].start,
                            self.words[start + len - 1].end,
                        )
                    });
                    start += len;
                }
                None => start += 1,
            }
        }
    }

    /// Существительные в родительном подряд; прилагательные и числительные в том же
    /// падеже между ними цепочку не прерывают.
    fn find_genitive_chains(&self, out: &mut Vec<Raw>) {
        let mut start: Option<usize> = None;
        let mut nouns = 0;
        let mut last_noun = 0;
        let mut close = |start: &mut Option<usize>, nouns: &mut usize, last_noun: usize| {
            if let Some(first) = start.take()
                && *nouns >= GENITIVE_CHAIN
            {
                out.push(Raw::new(
                    Kind::GenitiveChain,
                    self.words[first].start,
                    self.words[last_noun].end,
                ));
            }
            *nouns = 0;
        };
        for (i, p) in self.parsed.iter().enumerate() {
            let genitive = p.case.as_deref() == Some("Gen");
            let noun = genitive && matches!(p.pos.as_str(), "NOUN" | "PROPN");
            let modifier = genitive && matches!(p.pos.as_str(), "ADJ" | "DET" | "NUM");
            if start.is_some() && !self.adjacent(i - 1) {
                close(&mut start, &mut nouns, last_noun);
            }
            if noun || modifier {
                start.get_or_insert(i);
                if noun {
                    nouns += 1;
                    last_noun = i;
                }
            } else {
                close(&mut start, &mut nouns, last_noun);
            }
        }
        close(&mut start, &mut nouns, last_noun);
    }
}

fn is_nominalization(lemma: &str) -> bool {
    (lemma.ends_with("ние") || lemma.ends_with("тие"))
        && lemma.chars().count() >= 6
        && !NOT_NOMINALIZATIONS.contains(&lemma)
}

/// Две и больше заглавных буквы без строчных: «ВВП», «ИЛИ»; римские числа не в счёт.
fn is_all_caps(form: &str) -> bool {
    let letters: Vec<char> = form.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2
        && letters.iter().all(|c| c.is_uppercase())
        && !letters
            .iter()
            .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary() -> Dictionary {
        Dictionary::bundled().unwrap()
    }

    fn found(report: &Report, kind: Kind) -> Vec<&str> {
        report
            .findings
            .iter()
            .filter(|f| f.kind == kind)
            .map(|f| f.text.as_str())
            .collect()
    }

    const BUREAUCRATIC: &str = "В целях обеспечения реализации мероприятий по повышению эффективности \
        деятельности органов исполнительной власти субъектов Российской Федерации осуществляется \
        проведение мониторинга. Результаты проведённого мониторинга направляются в МВД.";

    #[test]
    fn finds_all_kinds_in_bureaucratic_text() {
        let r = check(&dictionary(), BUREAUCRATIC);
        assert_eq!(
            found(&r, Kind::GenitiveChain),
            [
                "обеспечения реализации мероприятий",
                "эффективности деятельности органов исполнительной власти субъектов Российской Федерации",
            ]
        );
        assert_eq!(
            found(&r, Kind::Bureaucratese),
            ["В целях", "реализации", "мероприятий", "осуществляется"]
        );
        assert_eq!(
            found(&r, Kind::Nominalization),
            ["обеспечения", "повышению", "проведение"]
        );
        assert_eq!(found(&r, Kind::Passive), ["проведённого"]);
        assert_eq!(found(&r, Kind::Abbreviation), ["МВД"]);
        assert_eq!(r.counts.genitive_chain, 2);
        assert_eq!(r.words, 24);
    }

    fn suggestions(report: &Report) -> Vec<(&str, &str)> {
        report
            .findings
            .iter()
            .filter(|f| f.kind == Kind::Bureaucratese)
            .map(|f| (f.text.as_str(), f.suggestion.unwrap_or("")))
            .collect()
    }

    #[test]
    fn bureaucratese_comes_with_suggestions() {
        let r = check(
            &dictionary(),
            "В целях экономии денежных средств оплата производится в полном объёме. \
             На данный момент при наличии билета проход свободный.",
        );
        assert_eq!(
            suggestions(&r),
            [
                ("В целях", "чтобы"),
                ("денежных средств", "деньги"),
                (
                    "производится",
                    "назовите, кто делает: «оплата производится» → «вы платите»"
                ),
                ("в полном объёме", "полностью"),
                ("На данный момент", "сейчас"),
                ("при наличии", "если есть"),
            ]
        );
        // Слово внутри оборота («данный», «наличии») отдельно не считается.
        assert_eq!(r.counts.bureaucratese, 6);
        // У остальных видов замен нет.
        assert!(
            r.findings
                .iter()
                .all(|f| (f.kind == Kind::Bureaucratese) == f.suggestion.is_some())
        );
    }

    #[test]
    fn bureaucratese_dictionary_is_valid() {
        let dict = &*BUREAUCRATESE;
        assert!(dict.words.len() >= 40 && dict.forms.len() >= 40 && !dict.lemmas.is_empty());
        let bad = [
            ("[слова]\nв целях = чтобы", "одно слово"),
            ("[слова]\nданный = этот\nданный = тот", "уже есть"),
            ("[обороты]\nв целях чтобы", "шаблон = замена"),
            ("[прочее]\nв целях = чтобы", "неизвестный раздел"),
            ("в целях = чтобы", "до первого раздела"),
        ];
        for (source, error) in bad {
            let e = parse_bureaucratese(source).unwrap_err();
            assert!(e.contains(error), "{source:?}: {e}");
        }
    }

    #[test]
    fn plain_text_has_no_findings() {
        let r = check(
            &dictionary(),
            "Мама мыла раму. Папа читал газету, а кот спал на окне.",
        );
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }

    #[test]
    fn punctuation_breaks_chains_and_phrases() {
        let r = check(
            &dictionary(),
            "Список органов, министерств, ведомств. В, целях проверки.",
        );
        assert!(found(&r, Kind::GenitiveChain).is_empty());
        assert!(found(&r, Kind::Bureaucratese).is_empty());
    }

    #[test]
    fn offsets_are_utf16() {
        let text = "👋 Данный документ.";
        let r = check(&dictionary(), text);
        let f = &r.findings[0];
        assert_eq!((f.kind, f.text.as_str()), (Kind::Bureaucratese, "Данный"));
        // 👋 — две единицы UTF-16 и пробел.
        assert_eq!((f.start, f.end), (3, 9));
    }

    #[test]
    fn abbreviations_but_not_shouted_words_or_roman_numerals() {
        let r = check(
            &dictionary(),
            "ВНИМАНИЕ: ДАННЫЕ НЕ ПЕРЕДАЮТСЯ. Отчёт МВД о ВВП за XIX век и данные ОМС.",
        );
        assert_eq!(found(&r, Kind::Abbreviation), ["МВД", "ВВП", "ОМС"]);
        assert!(!is_all_caps("XIX"));
        assert!(!is_all_caps("Ростов"));
        assert!(!is_all_caps("Я"));
    }
}
