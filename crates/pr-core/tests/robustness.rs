//! Устойчивость оценки к оформлению текста: регистр букв не должен её менять.

mod common;

use common::{corpus_dir, skip_without_corpus};
use pr_core::{legacy, readability};

fn average(text: &str) -> f64 {
    readability::measure(text).indexes.unwrap().average.value
}

#[test]
fn uppercase_text_gets_nearly_the_same_grade() {
    let Some(dir) = corpus_dir() else {
        return skip_without_corpus();
    };
    let csv = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../calibration/corpus.csv"
    ))
    .unwrap();
    let (mut new_shift, mut old_shift, mut n) = (0.0, 0.0, 0.0);
    for line in csv.lines().skip(1) {
        let path = line.split(',').next().unwrap();
        let text = std::fs::read_to_string(dir.join(path)).unwrap();
        let upper = text.to_uppercase();
        new_shift += (average(&text) - average(&upper)).abs();
        let smog = |t: &str| legacy::measure(t).indexes.index_smog;
        old_shift += (smog(&text) - smog(&upper)).abs();
        n += 1.0;
    }
    eprintln!(
        "сдвиг оценки от заглавных букв: новый {:.2}, старый {:.2} лет",
        new_shift / n,
        old_shift / n
    );
    assert!(new_shift / n < 0.5);
}
