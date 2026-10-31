//! Проверка `plainrussian` как программы: аргументы, форматы, ошибки.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const TEXT: &str =
    "Мама мыла раму. Папа читал газету. Высокопревосходительство приехало на заседание.";

fn run(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_plainrussian"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

/// Временный каталог с файлами; удаляется в конце теста.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("pr-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn text_from_stdin() {
    let out = run(&["-"], Some(TEXT.as_bytes()));
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("-: "), "{stdout}");
    assert!(stdout.contains(" года обучения — "), "{stdout}");
    assert!(stdout.contains("слов 10, предложений 3"), "{stdout}");
}

#[test]
fn json_matches_the_library() {
    let out = run(&["--format", "json", "-"], Some(TEXT.as_bytes()));
    let line: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let expected = serde_json::to_value(pr_core::readability::measure(TEXT)).unwrap();
    assert_eq!(line["path"], "-");
    assert_eq!(line["measurement"], expected);
}

#[test]
fn csv_for_a_directory_including_windows_1251() {
    let dir = TempDir::new("csv");
    std::fs::create_dir(dir.0.join("sub")).unwrap();
    std::fs::write(dir.0.join("a.txt"), TEXT).unwrap();
    let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode(TEXT);
    std::fs::write(dir.0.join("sub/b.md"), &cp1251).unwrap();
    std::fs::write(dir.0.join("skip.docx"), b"not text").unwrap();

    let out = run(&["--format", "csv", dir.0.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(lines[0].starts_with("path,average,grade,"));
    assert!(lines[1].contains("a.txt,"));
    assert!(lines[2].contains("b.md,"));
    // Тот же текст в windows-1251 даёт ту же оценку.
    let tail = |l: &str| l.split_once(',').unwrap().1.to_string();
    assert_eq!(tail(lines[1]), tail(lines[2]));
}

#[test]
fn missing_file_is_reported_but_others_are_measured() {
    let dir = TempDir::new("missing");
    let ok = dir.0.join("ok.txt");
    std::fs::write(&ok, TEXT).unwrap();
    let out = run(&[ok.to_str().unwrap(), "нет-такого.txt"], None);
    assert!(!out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("ok.txt: "));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("нет-такого.txt")
    );
}

#[test]
fn very_simple_text_is_below_the_scale() {
    let out = run(&["-"], Some("Кот спал. Пёс лаял. Дом стоял.".as_bytes()));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("меньше 1 года обучения"), "{stdout}");
}

#[test]
fn text_without_words() {
    let out = run(&["-"], Some(b"123 ... 456"));
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "-: в тексте нет слов"
    );
}

#[test]
fn clarity_in_text_json_and_csv() {
    let bureaucratic =
        "В целях обеспечения реализации мероприятий осуществляется проведение мониторинга.";
    let out = run(&["--clarity", "-"], Some(bureaucratic.as_bytes()));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("ясный язык: цепочки родительных 1, отглагольные 2, канцеляризмы 3"),
        "{stdout}"
    );

    let out = run(
        &["--clarity", "--format", "json", "-"],
        Some(bureaucratic.as_bytes()),
    );
    let line: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(line["clarity"]["counts"]["genitive_chain"], 1);
    assert_eq!(line["clarity"]["findings"][0]["text"], "В целях");

    let out = run(
        &["--clarity", "--format", "csv", "-"],
        Some(bureaucratic.as_bytes()),
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines[0].ends_with(",genitive_chain,nominalization,bureaucratese,passive,abbreviation")
    );
    assert!(lines[1].ends_with(",1,2,3,0,0"), "{}", lines[1]);
}
