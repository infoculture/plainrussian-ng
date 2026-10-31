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

/// Канцеляризмы-слова, по лемме.
const BUREAUCRATIC_LEMMAS: &[&str] = &[
    "вышеизложенный",
    "вышеуказанный",
    "вышеупомянутый",
    "данный",
    "мероприятие",
    "надлежащий",
    "нижеследующий",
    "осуществление",
    "осуществлять",
    "осуществляться",
    "посредством",
    "функционирование",
    "функционировать",
    "являться",
];

/// Канцелярские обороты, по словоформам подряд без знаков препинания между ними.
const BUREAUCRATIC_PHRASES: &[&[&str]] = &[
    &["в", "адрес"],
    &["в", "настоящее", "время"],
    &["в", "отношении"],
    &["в", "рамках"],
    &["в", "связи", "с"],
    &["в", "соответствии", "с"],
    &["в", "целях"],
    &["в", "части"],
    &["имеет", "место"],
    &["имеют", "место"],
    &["на", "основании"],
    &["на", "сегодняшний", "день"],
    &["по", "линии"],
    &["по", "причине"],
    &["при", "наличии"],
    &["с", "целью"],
    &["со", "стороны"],
];

/// Существительные на -ние и -тие, которые обычно не канцелярит: предметы и
/// привычные слова. Остальные отмечаются как отглагольные.
const NOT_NOMINALIZATIONS: &[&str] = &[
    "внимание",
    "желание",
    "задание",
    "занятие",
    "здание",
    "знание",
    // Не отглагольное, но канцелярит: есть в BUREAUCRATIC_LEMMAS.
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

    findings.sort_by_key(|&(kind, start, _)| (start, kind));
    let mut counts = Counts::default();
    let findings = findings
        .into_iter()
        .map(|(kind, start, end)| {
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
            }
        })
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

type Raw = (Kind, usize, usize);

impl Sentence<'_> {
    fn find(&self, out: &mut Vec<Raw>) {
        for (word, p) in self.words.iter().zip(self.parsed) {
            if p.abbreviation {
                out.push((Kind::Abbreviation, word.start, word.end));
            }
            if p.pos == "NOUN" && is_nominalization(&p.lemma) {
                out.push((Kind::Nominalization, word.start, word.end));
            }
            if p.pos == "VERB" && p.passive {
                out.push((Kind::Passive, word.start, word.end));
            }
            if BUREAUCRATIC_LEMMAS.contains(&p.lemma.as_str()) {
                out.push((Kind::Bureaucratese, word.start, word.end));
            }
        }
        self.find_phrases(out);
        self.find_genitive_chains(out);
    }

    /// Между словами только пробелы: знак препинания разрывает оборот и цепочку.
    fn adjacent(&self, i: usize) -> bool {
        self.chars[self.words[i].end..self.words[i + 1].start]
            .iter()
            .all(|c| c.is_whitespace())
    }

    fn find_phrases(&self, out: &mut Vec<Raw>) {
        let lower: Vec<String> = self.forms.iter().map(|f| f.to_lowercase()).collect();
        for start in 0..lower.len() {
            for phrase in BUREAUCRATIC_PHRASES {
                let end = start + phrase.len();
                if end <= lower.len()
                    && lower[start..end].iter().zip(*phrase).all(|(w, p)| w == p)
                    && (start..end - 1).all(|i| self.adjacent(i))
                {
                    out.push((
                        Kind::Bureaucratese,
                        self.words[start].start,
                        self.words[end - 1].end,
                    ));
                }
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
                out.push((
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
            ["В целях", "мероприятий", "осуществляется"]
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
