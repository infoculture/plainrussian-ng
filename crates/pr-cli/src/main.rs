//! `plainrussian` — оценка простоты русскоязычных текстов из командной строки.
//!
//!     plainrussian статья.txt                  краткая сводка
//!     plainrussian --format csv тексты/ > a.csv все .txt и .md в каталоге, таблицей
//!     cat статья.txt | plainrussian --format json -

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use pr_core::readability::{self, Measurement};

#[derive(Parser)]
#[command(
    name = "plainrussian",
    version,
    about = "Оценка простоты (читаемости) русскоязычных текстов"
)]
struct Args {
    /// Файлы и каталоги (в каталогах берутся все .txt и .md); «-» — стандартный ввод.
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Формат вывода: text — сводка для человека, json — по строке JSON на текст, csv — таблица.
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
    Csv,
}

const CSV_HEADER: &str = "path,average,grade,fk,cl,smog,dc,ari,sentences,words,syllables,\
letters,complex_words,numbers,chars,calibration";

fn main() -> ExitCode {
    let args = Args::parse();
    let mut inputs = Vec::new();
    let mut failed = false;
    for path in &args.paths {
        if let Err(e) = collect(path, &mut inputs) {
            eprintln!("plainrussian: {}: {e}", path.display());
            failed = true;
        }
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if matches!(args.format, Format::Csv) {
        let _ = writeln!(out, "{CSV_HEADER}");
    }
    for input in inputs {
        let name = input.display().to_string();
        let text = match read_text(&input) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("plainrussian: {name}: {e}");
                failed = true;
                continue;
            }
        };
        let m = readability::measure(&text);
        let line = match args.format {
            Format::Text => text_line(&name, &m),
            Format::Json => serde_json::json!({ "path": name, "measurement": m }).to_string(),
            Format::Csv => csv_line(&name, &m),
        };
        // Закрытый канал (например, `| head`) — не ошибка оценки.
        if writeln!(out, "{line}").is_err() {
            break;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Раскрывает каталоги в отсортированный список файлов .txt и .md.
fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.as_os_str() == "-" || !path.is_dir() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            collect(&entry, out)?;
        } else if entry
            .extension()
            .is_some_and(|ext| ext == "txt" || ext == "md")
        {
            out.push(entry);
        }
    }
    Ok(())
}

/// Текст в UTF-8; если файл не в UTF-8, это почти наверняка старый русский файл в windows-1251.
fn read_text(path: &Path) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    if path.as_os_str() == "-" {
        std::io::stdin().read_to_end(&mut bytes)?;
    } else {
        bytes = std::fs::read(path)?;
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => encoding_rs::WINDOWS_1251
            .decode(e.as_bytes())
            .0
            .into_owned(),
    };
    Ok(text
        .strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(text))
}

fn text_line(name: &str, m: &Measurement) -> String {
    let c = &m.counts;
    match &m.indexes {
        Some(ix) => format!(
            "{name}: {} — {} (слов {}, предложений {})",
            years(ix.average.value),
            ix.average.grade,
            c.words,
            c.sentences
        ),
        None => format!("{name}: в тексте нет слов"),
    }
}

/// Шкала размечена от 1 до 17 («17 и больше»); за её пределами формулы экстраполируют.
fn years(value: f64) -> String {
    if value < 1.0 {
        "меньше 1 года обучения".to_string()
    } else if value >= 17.0 {
        "17 и больше лет обучения".to_string()
    } else {
        // Дробное число: «4,5 года».
        format!("{:.1} года обучения", value).replace('.', ",")
    }
}

fn csv_line(name: &str, m: &Measurement) -> String {
    let c = &m.counts;
    let number = |v: Option<f64>| v.map(|v| format!("{v:.4}")).unwrap_or_default();
    let ix = m.indexes.as_ref();
    [
        csv_field(name),
        number(ix.map(|i| i.average.value)),
        csv_field(ix.map(|i| i.average.grade.as_str()).unwrap_or("")),
        number(ix.map(|i| i.fk.value)),
        number(ix.map(|i| i.cl.value)),
        number(ix.map(|i| i.smog.value)),
        number(ix.map(|i| i.dc.value)),
        number(ix.map(|i| i.ari.value)),
        c.sentences.to_string(),
        c.words.to_string(),
        c.syllables.to_string(),
        c.letters.to_string(),
        c.complex_words.to_string(),
        c.numbers.to_string(),
        c.chars.to_string(),
        m.calibration.version.to_string(),
    ]
    .join(",")
}

/// Поле CSV по RFC 4180: в кавычках, если в нём есть запятая, кавычка или перевод строки.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
