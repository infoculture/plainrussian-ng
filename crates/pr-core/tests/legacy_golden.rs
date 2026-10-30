//! Сверка legacy-алгоритма с Python-оригиналом.
//!
//! Фикстуры генерирует `tools/legacy-golden/generate.py`. Числа сравниваются
//! точно, без допуска: перенос должен совпадать бит в бит.
//!
//! Тексты корпуса в репозиторий не входят, см. `tests/common`.

mod common;

use common::{corpus_dir, corpus_text, golden, skip_without_corpus};
use pr_core::legacy;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct EdgeCase {
    name: String,
    text: String,
    expected: Value,
}

#[derive(Deserialize)]
struct CorpusCase {
    path: String,
    sha256: String,
    expected: Value,
}

/// Сравнивает JSON по структуре. Python отдаёт целый 0 там, где Rust отдаёт 0.0,
/// поэтому числа сравниваются как f64.
fn assert_same(case: &str, at: &str, expected: &Value, actual: &Value) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            let mut ek: Vec<_> = e.keys().collect();
            let mut ak: Vec<_> = a.keys().collect();
            ek.sort();
            ak.sort();
            assert_eq!(ek, ak, "{case}: разные ключи в {at}");
            for (k, ev) in e {
                assert_same(case, &format!("{at}.{k}"), ev, &a[k]);
            }
        }
        (Value::Number(e), Value::Number(a)) => {
            let (e, a) = (e.as_f64().unwrap(), a.as_f64().unwrap());
            assert!(e == a, "{case}: {at}: Python {e:?}, Rust {a:?}");
        }
        _ => assert_eq!(expected, actual, "{case}: {at}"),
    }
}

fn check(case: &str, text: &str, expected: &Value) {
    let measurement = legacy::measure(text);
    let actual = serde_json::to_value(&measurement).unwrap();
    assert_same(case, "$", expected, &actual);
    check_annotation(case, text, &measurement.metrics);
}

/// Разметка для подсветки должна объяснять те же числа, что показывает `measure`.
fn check_annotation(case: &str, text: &str, m: &legacy::Metrics) {
    let a = legacy::annotate(text);
    let len = text.encode_utf16().count();

    assert_eq!(a.words.len() as u64, m.n_words, "{case}: слова");
    let syllables: u64 = a.words.iter().map(|w| w.syllables).sum();
    assert_eq!(syllables, m.n_syllabes, "{case}: слоги");
    let complex = a
        .words
        .iter()
        .filter(|w| w.syllables > legacy::COMPLEX_SYL_FACTOR)
        .count();
    assert_eq!(complex as u64, m.n_complex_words, "{case}: сложные слова");
    let terminated = a.sentences.iter().filter(|s| s.terminated).count();
    assert_eq!(terminated as u64, m.n_sentences, "{case}: предложения");
    let in_sentences: u64 = a.sentences.iter().map(|s| s.words).sum();
    assert_eq!(in_sentences, m.n_words, "{case}: слова по предложениям");
    assert!(
        a.sentences.iter().filter(|s| !s.terminated).count() <= 1,
        "{case}: незакрытым может быть только последнее предложение"
    );

    let mut prev = 0;
    for w in &a.words {
        assert!(
            prev <= w.start && w.start < w.end && w.end <= len,
            "{case}: {w:?}"
        );
        prev = w.end;
    }
    prev = 0;
    for s in &a.sentences {
        assert!(
            prev <= s.start && s.start < s.end && s.end <= len,
            "{case}: {s:?}"
        );
        prev = s.end;
    }
}

#[test]
fn edge_cases_match_python() {
    let cases: Vec<EdgeCase> = serde_json::from_str(&golden("legacy_edge_cases.json")).unwrap();
    assert!(!cases.is_empty());
    for case in &cases {
        check(&case.name, &case.text, &case.expected);
    }
}

#[test]
fn corpus_matches_python() {
    let Some(dir) = corpus_dir() else {
        return skip_without_corpus();
    };
    let cases: Vec<CorpusCase> = serde_json::from_str(&golden("legacy_corpus.json")).unwrap();
    assert!(!cases.is_empty());
    for case in &cases {
        let text = corpus_text(&dir, &case.path, &case.sha256);
        check(&case.path, &text, &case.expected);
    }
}
