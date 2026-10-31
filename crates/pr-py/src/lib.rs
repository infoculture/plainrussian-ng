//! Модуль `plainrussian._core` для Python. Результаты отдаются строками JSON,
//! в словари их превращает `python/plainrussian/__init__.py`: так привязка не
//! зависит от того, как в очередной версии pyo3 строятся объекты Python.

use pr_core::sentenize::sentenize as split_sentences;
use pr_core::{legacy, readability};
use pyo3::prelude::*;
use serde::Serialize;

/// Слово сложное, если в нём больше стольких слогов.
const COMPLEX_SYLLABLES: u64 = readability::COMPLEX_SYLLABLES;

#[derive(Serialize)]
struct Sentence<'a> {
    text: &'a str,
    words: u64,
}

#[derive(Serialize)]
struct Word<'a> {
    text: &'a str,
    syllables: u64,
}

#[derive(Serialize)]
struct Details<'a> {
    measurement: readability::Measurement,
    sentences: Vec<Sentence<'a>>,
    complex_words: Vec<Word<'a>>,
}

/// Оценка текста: счётчики, признаки, индексы, калибровка.
#[pyfunction]
fn measure_json(text: &str) -> String {
    to_json(&readability::measure(text))
}

/// Оценка плюс предложения с числом слов и сложные слова — уже строками.
/// Смещения разметки в ядре в единицах UTF-16 (для JavaScript), а в Python
/// строки индексируются по символам, поэтому фрагменты вырезаются здесь.
#[pyfunction]
fn details_json(text: &str) -> String {
    let analysis = readability::analyze(text);
    let slicer = Utf16Slicer::new(text);
    let sentences = analysis
        .annotation
        .sentences
        .iter()
        .map(|s| Sentence {
            text: slicer.slice(s.start, s.end),
            words: s.words,
        })
        .collect();
    let complex_words = analysis
        .annotation
        .words
        .iter()
        .filter(|w| w.syllables > COMPLEX_SYLLABLES)
        .map(|w| Word {
            text: slicer.slice(w.start, w.end),
            syllables: w.syllables,
        })
        .collect();
    to_json(&Details {
        measurement: analysis.measurement,
        sentences,
        complex_words,
    })
}

/// Предложения по правилам razdel.
#[pyfunction]
fn sentenize(text: &str) -> Vec<String> {
    split_sentences(text)
        .into_iter()
        .map(|r| text[r].to_string())
        .collect()
}

/// Оценка в формате старого API plainrussian.ru (`/api/1.0/ru/measure/`).
#[pyfunction]
fn measure_legacy_json(text: &str) -> String {
    to_json(&legacy::measure(text))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(measure_json, m)?)?;
    m.add_function(wrap_pyfunction!(details_json, m)?)?;
    m.add_function(wrap_pyfunction!(sentenize, m)?)?;
    m.add_function(wrap_pyfunction!(measure_legacy_json, m)?)?;
    Ok(())
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("результат всегда сериализуется в JSON")
}

/// Перевод смещений UTF-16 в байтовые для вырезания фрагментов.
struct Utf16Slicer<'a> {
    text: &'a str,
    /// Байтовое начало каждой единицы UTF-16; у второй половины суррогатной пары — та же позиция.
    bytes: Vec<usize>,
}

impl<'a> Utf16Slicer<'a> {
    fn new(text: &'a str) -> Self {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        for (i, c) in text.char_indices() {
            for _ in 0..c.len_utf16() {
                bytes.push(i);
            }
        }
        bytes.push(text.len());
        Self { text, bytes }
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.text[self.bytes[start]..self.bytes[end]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_cut_fragments_by_characters() {
        let json: serde_json::Value = serde_json::from_str(&details_json(
            "Привет 👋 мир! Высокопревосходительство спит.",
        ))
        .unwrap();
        assert_eq!(json["sentences"][0]["text"], "Привет 👋 мир!");
        assert_eq!(
            json["sentences"][1]["text"],
            "Высокопревосходительство спит."
        );
        assert_eq!(json["complex_words"][0]["text"], "Высокопревосходительство");
        assert_eq!(json["complex_words"][0]["syllables"], 9);
    }
}
