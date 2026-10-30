//! Сверка деления на предложения с Python-пакетом razdel.
//!
//! Фикстуры генерирует `tools/razdel-golden/generate.py`. Границы должны
//! совпадать точно. Смещения в фикстурах — в символах, как у строк Python.

mod common;

use common::{corpus_dir, corpus_text, golden, skip_without_corpus};
use pr_core::sentenize::sentenize;
use serde::Deserialize;

#[derive(Deserialize)]
struct Cases {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    text: String,
    sentences: Vec<[usize; 2]>,
}

#[derive(Deserialize)]
struct Corpus {
    texts: Vec<CorpusText>,
}

#[derive(Deserialize)]
struct CorpusText {
    path: String,
    sha256: String,
    sentences: Vec<[usize; 2]>,
}

/// Границы предложений в символах.
fn sentences_in_chars(text: &str) -> Vec<[usize; 2]> {
    let char_at = |byte: usize| text[..byte].chars().count();
    sentenize(text)
        .into_iter()
        .map(|r| [char_at(r.start), char_at(r.end)])
        .collect()
}

fn check(name: &str, text: &str, expected: &[[usize; 2]]) {
    let actual = sentences_in_chars(text);
    if actual == expected {
        return;
    }
    // Показываем первое расхождение текстом, иначе по числам ничего не понять.
    let chars: Vec<char> = text.chars().collect();
    let show = |s: &[usize; 2]| chars[s[0]..s[1]].iter().collect::<String>();
    let i = actual
        .iter()
        .zip(expected)
        .take_while(|(a, e)| a == e)
        .count();
    panic!(
        "{name}: расхождение в предложении {i}\n  razdel: {:?}\n  Rust:   {:?}",
        expected.get(i).map(show),
        actual.get(i).map(show),
    );
}

#[test]
fn cases_match_razdel() {
    let cases: Cases = serde_json::from_str(&golden("razdel_cases.json")).unwrap();
    assert!(!cases.cases.is_empty());
    for case in &cases.cases {
        check(&case.text, &case.text, &case.sentences);
    }
}

#[test]
fn corpus_matches_razdel() {
    let Some(dir) = corpus_dir() else {
        return skip_without_corpus();
    };
    let corpus: Corpus = serde_json::from_str(&golden("razdel_corpus.json")).unwrap();
    assert!(!corpus.texts.is_empty());
    for t in &corpus.texts {
        let text = corpus_text(&dir, &t.path, &t.sha256);
        check(&t.path, &text, &t.sentences);
    }
}
