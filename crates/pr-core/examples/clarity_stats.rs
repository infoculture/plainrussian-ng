//! Плотность находок проверок ясного языка по уровням корпуса: на 100 слов.
//! Проверяет, что проверки отличают детские тексты от канцелярита. Затем — самые
//! частые канцеляризмы каждого уровня: по ним видно, какие слова словаря
//! (`data/bureaucratese.txt`) срабатывают и в простых текстах.
//!
//!     cargo run -p pr-core --features clarity --example clarity_stats -- <каталог textsbygrade>

use std::collections::BTreeMap;
use std::path::PathBuf;

use pr_core::clarity::{Dictionary, Kind, check};

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("укажите каталог корпуса"));
    let dictionary = Dictionary::bundled().expect("словарь rumorph");
    // Группа уровней → [слова, отглагольные, страдательные, цепочки, канцеляризмы, аббревиатуры].
    let mut groups: BTreeMap<&str, [u64; 6]> = BTreeMap::new();
    // Группа уровней → канцеляризм в нижнем регистре → сколько раз найден.
    let mut bureaucratese: BTreeMap<&str, BTreeMap<String, u64>> = BTreeMap::new();
    let started = std::time::Instant::now();
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        let grade: u32 = dir.file_name().unwrap().to_str().unwrap().parse().unwrap();
        let group = match grade {
            1..=4 => "1–4",
            5..=9 => "5–9",
            10..=12 => "10–12",
            _ => "15–17",
        };
        for file in std::fs::read_dir(&dir).unwrap() {
            let text = std::fs::read_to_string(file.unwrap().path()).unwrap();
            let r = check(&dictionary, &text);
            for f in r.findings.iter().filter(|f| f.kind == Kind::Bureaucratese) {
                *bureaucratese
                    .entry(group)
                    .or_default()
                    .entry(
                        f.text
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .to_lowercase(),
                    )
                    .or_default() += 1;
            }
            let g = groups.entry(group).or_default();
            let c = &r.counts;
            for (slot, n) in g.iter_mut().zip([
                r.words,
                c.nominalization,
                c.passive,
                c.genitive_chain,
                c.bureaucratese,
                c.abbreviation,
            ]) {
                *slot += n;
            }
        }
    }
    println!("уровни\tслов\tотглаг.\tстрад.\tцепочки\tканц.\tаббр.  (на 100 слов)");
    for (group, g) in &groups {
        let per = |n: u64| 100.0 * n as f64 / g[0] as f64;
        println!(
            "{group}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
            g[0],
            per(g[1]),
            per(g[2]),
            per(g[3]),
            per(g[4]),
            per(g[5])
        );
    }
    for (group, found) in &bureaucratese {
        let mut top: Vec<_> = found.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = top
            .iter()
            .take(12)
            .map(|(w, n)| format!("{w} {n}"))
            .collect();
        println!("канцеляризмы {group}: {}", top.join(", "));
    }
    eprintln!("время: {:?}", started.elapsed());
}
