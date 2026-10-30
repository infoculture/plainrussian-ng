//! Выводит счётчики нового и старого алгоритма для каждого текста корпуса,
//! по одной строке JSON на текст. Из этих данных `calibration/fit.py` подбирает
//! коэффициенты, поэтому сами тексты для калибровки не нужны.
//!
//!     cargo run -p pr-core --example corpus_features -- <каталог textsbygrade> > calibration/features.jsonl

use std::path::{Path, PathBuf};

use pr_core::{legacy, readability};
use serde_json::json;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "txt") {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("укажите каталог корпуса"));
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let rel = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let new = readability::measure(&text).counts;
        let old = legacy::measure(&text).metrics;
        let line = json!({
            "path": rel,
            "counts": new,
            "legacy": {
                "sentences": old.n_sentences,
                "words": old.n_words,
                "syllables": old.n_syllabes,
                "letters": old.letters,
                "complex_words": old.n_complex_words,
            },
        });
        println!("{line}");
    }
}
