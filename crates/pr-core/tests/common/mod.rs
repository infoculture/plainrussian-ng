//! Общее для golden-тестов: чтение фикстур и текстов корпуса.
//!
//! Тексты корпуса в репозиторий не входят. Тесты берут их из
//! `PLAINRUSSIAN_LEGACY_CORPUS` или из соседнего репозитория plainrussian.

#![allow(dead_code)] // каждый тестовый файл использует свою часть

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn corpus_dir() -> Option<PathBuf> {
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

/// Читает текст корпуса и проверяет, что он тот же, по которому сделана фикстура.
pub fn corpus_text(dir: &Path, path: &str, sha256: &str) -> String {
    let bytes = std::fs::read(dir.join(path)).unwrap();
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        actual, sha256,
        "{path}: текст отличается от того, что в фикстуре"
    );
    String::from_utf8(bytes).unwrap()
}

pub fn skip_without_corpus() {
    eprintln!("пропуск: корпус не найден, задайте PLAINRUSSIAN_LEGACY_CORPUS");
}
