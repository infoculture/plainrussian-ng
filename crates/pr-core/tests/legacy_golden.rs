//! Сверка legacy-алгоритма с Python-оригиналом.
//!
//! Фикстуры генерирует `tools/legacy-golden/generate.py`. Числа сравниваются
//! точно, без допуска: перенос должен совпадать бит в бит.
//!
//! Тексты корпуса в репозиторий не входят. Тест берёт их из
//! `PLAINRUSSIAN_LEGACY_CORPUS` или из соседнего репозитория plainrussian.

use std::path::{Path, PathBuf};

use pr_core::legacy;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

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

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
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
    let actual = serde_json::to_value(legacy::measure(text)).unwrap();
    assert_same(case, "$", expected, &actual);
}

#[test]
fn edge_cases_match_python() {
    let cases: Vec<EdgeCase> = serde_json::from_str(&golden("legacy_edge_cases.json")).unwrap();
    assert!(!cases.is_empty());
    for case in &cases {
        check(&case.name, &case.text, &case.expected);
    }
}

fn corpus_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("PLAINRUSSIAN_LEGACY_CORPUS") {
        let dir = PathBuf::from(dir);
        assert!(
            dir.is_dir(),
            "PLAINRUSSIAN_LEGACY_CORPUS: нет каталога {}",
            dir.display()
        );
        return Some(dir);
    }
    let sibling = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../plainrussian/api/textmetric/textsbygrade");
    sibling.is_dir().then_some(sibling)
}

#[test]
fn corpus_matches_python() {
    let Some(dir) = corpus_dir() else {
        eprintln!("пропуск: корпус не найден, задайте PLAINRUSSIAN_LEGACY_CORPUS");
        return;
    };
    let cases: Vec<CorpusCase> = serde_json::from_str(&golden("legacy_corpus.json")).unwrap();
    assert!(!cases.is_empty());
    for case in &cases {
        let bytes = std::fs::read(dir.join(&case.path)).unwrap();
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            sha256, case.sha256,
            "{}: текст отличается от того, что в фикстуре",
            case.path
        );
        let text = String::from_utf8(bytes).unwrap();
        check(&case.path, &text, &case.expected);
    }
}
